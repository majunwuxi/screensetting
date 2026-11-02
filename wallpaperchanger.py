import sys
import os
import ctypes
import requests
import logging
import subprocess
import winreg
from datetime import datetime
from PyQt6.QtWidgets import (QApplication, QMainWindow, QWidget, QVBoxLayout, 
                            QHBoxLayout, QLineEdit, QPushButton, QLabel, 
                            QScrollArea, QGridLayout, QMessageBox, QFrame,
                            QSizePolicy)
from PyQt6.QtCore import Qt, QThread, pyqtSignal, QSize
from PyQt6.QtGui import QPixmap, QColor, QPalette, QIcon, QFontDatabase, QFont

# 配置日志
logging.basicConfig(
    level=logging.DEBUG,
    format='%(asctime)s - %(levelname)s - %(message)s',
    handlers=[
        logging.StreamHandler(sys.stdout),
        logging.FileHandler(f'wallpaper_app_{datetime.now().strftime("%Y%m%d")}.log')
    ]
)
logger = logging.getLogger(__name__)

# 材料设计颜色
ACCENT_COLOR = "#1976D2"  # Material Blue 700
HOVER_COLOR = "#1565C0"   # Material Blue 800
BG_COLOR = "#FFFFFF"
TEXT_COLOR = "#212121"
CARD_BG_COLOR = "#F5F5F5"

class MaterialLineEdit(QLineEdit):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.setStyleSheet("""\
            QLineEdit {
                padding: 8px 16px;
                border: 2px solid #E0E0E0;
                border-radius: 4px;
                background-color: white;
                font-size: 14px;
            }
            QLineEdit:focus {
                border: 2px solid """ + ACCENT_COLOR + """;
            }
        """)
        self.setMinimumHeight(40)

class MaterialButton(QPushButton):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.setStyleSheet("""\
            QPushButton {
                background-color: """ + ACCENT_COLOR + """;
                color: white;
                border: none;
                padding: 8px 16px;
                border-radius: 4px;
                font-size: 14px;
                min-height: 40px;
            }
            QPushButton:hover {
                background-color: """ + HOVER_COLOR + """;
            }
            QPushButton:pressed {
                background-color: """ + ACCENT_COLOR + """;
            }
            QPushButton:disabled {
                background-color: #BDBDBD;
            }
        """)

class ImageCard(QFrame):
    def __init__(self, parent=None):
        super().__init__(parent)
        self.setStyleSheet("""\
            QFrame {
                background-color: """ + CARD_BG_COLOR + """;
                border-radius: 8px;
                margin: 8px;
                padding: 8px;
            }
        """)
        self.setSizePolicy(QSizePolicy.Policy.Fixed, QSizePolicy.Policy.Fixed)
        self.setFixedSize(280, 320)

class ImageDownloader(QThread):
    finished = pyqtSignal(str, str)
    
    def __init__(self, url, save_path):
        super().__init__()
        self.url = url
        self.save_path = save_path
        
    def run(self):
        try:
            logger.info(f"开始下载图片: {self.url}")
            response = requests.get(self.url)
            if response.status_code == 200:
                with open(self.save_path, 'wb') as f:
                    f.write(response.content)
                logger.info(f"图片下载成功: {self.save_path}")
                self.finished.emit(self.save_path, self.url)
            else:
                logger.error(f"图片下载失败: HTTP {response.status_code}")
        except Exception as e:
            logger.error(f"下载过程出错: {str(e)}")

def get_highest_resolution_url(image_data):
    """获取最高分辨率的图片URL"""
    urls = image_data['urls']
    if 'raw' in urls and urls['raw']:
        return urls['raw']
    elif 'full' in urls and urls['full']:
        return urls['full']
    elif 'regular' in urls and urls['regular']:
        return urls['regular']
    else:
        return urls['small']

class UnsplashAPI:
    def __init__(self, client_id):
        self.client_id = client_id
        self.base_url = "https://api.unsplash.com"
        logger.info(f"初始化 Unsplash API")
        
    def search_photos(self, query, per_page=20):
        logger.info(f"搜索图片，关键词: {query}")
        endpoint = f"{self.base_url}/search/photos"
        params = {
            "query": query,
            "per_page": per_page,
            "orientation": "landscape",
            "content_filter": "high"
        }
        headers = {"Authorization": f"Client-ID {self.client_id}"}
        
        try:
            response = requests.get(endpoint, params=params, headers=headers)
            if response.status_code == 200:
                results = response.json()["results"]
                logger.info(f"搜索成功，获取到 {len(results)} 张图片")
                return results
            else:
                logger.error(f"API请求失败: HTTP {response.status_code}")
                logger.error(f"响应内容: {response.text}")
                return []
        except Exception as e:
            logger.error(f"API请求出错: {str(e)}")
            return []

class WallpaperApp(QMainWindow):
    def __init__(self):
        super().__init__()
        self.api = UnsplashAPI("PtWrZwThqF0NYvw_XDTKwc4YOrRNWevrZQYfvJ02nLE")
        self.downloads_folder = os.path.join(os.path.expanduser("~"), "Pictures", "Wallpapers")
        os.makedirs(self.downloads_folder, exist_ok=True)
        logger.info(f"图片保存目录: {self.downloads_folder}")
        
        # 设置应用图标
        icon_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "icon.ico")
        if os.path.exists(icon_path):
            self.setWindowIcon(QIcon(icon_path))
        
        self.setup_ui()
        self.image_threads = []
        
    def setup_ui(self):
        self.setWindowTitle("壁纸设置工具")
        self.setMinimumSize(1200, 800)
        self.setStyleSheet(f"""\
            QMainWindow {{
                background-color: {BG_COLOR};
            }}
            QLabel {{
                color: {TEXT_COLOR};
                font-size: 14px;
            }}
        """)
        
        main_widget = QWidget()
        self.setCentralWidget(main_widget)
        layout = QVBoxLayout(main_widget)
        layout.setSpacing(16)
        layout.setContentsMargins(16, 16, 16, 16)
        
        # 标题
        title_label = QLabel("壁纸设置工具")
        title_label.setStyleSheet("""\
            font-size: 24px;
            font-weight: bold;
            margin-bottom: 16px;
            color: """ + TEXT_COLOR + """;
        """)
        layout.addWidget(title_label)
        
        # 搜索栏
        search_container = QWidget()
        search_container.setStyleSheet("""\
            QWidget {{
                background-color: white;
                border-radius: 8px;
                padding: 16px;
            }}
        """)
        search_layout = QHBoxLayout(search_container)
        search_layout.setContentsMargins(16, 16, 16, 16)
        
        self.search_input = MaterialLineEdit()
        self.search_input.setPlaceholderText("输入关键词搜索图片...")
        self.search_input.returnPressed.connect(self.search_images)
        
        search_button = MaterialButton("搜索")
        search_button.clicked.connect(self.search_images)
        
        search_layout.addWidget(self.search_input, 1)
        search_layout.addWidget(search_button)
        
        layout.addWidget(search_container)
        
        # 图片展示区域
        scroll = QScrollArea()
        scroll.setWidgetResizable(True)
        scroll.setStyleSheet("""\
            QScrollArea {{
                border: none;
                background-color: transparent;
            }}
            QScrollBar:vertical {{
                border: none;
                background: #F5F5F5;
                width: 8px;
                border-radius: 4px;
            }}
            QScrollBar::handle:vertical {{
                background: #BDBDBD;
                border-radius: 4px;
            }}
            QScrollBar::add-line:vertical {{
                height: 0px;
            }}
            QScrollBar::sub-line:vertical {{
                height: 0px;
            }}
        """)
        
        self.grid_widget = QWidget()
        self.grid_layout = QGridLayout(self.grid_widget)
        self.grid_layout.setSpacing(16)
        scroll.setWidget(self.grid_widget)
        
        layout.addWidget(scroll)
        
    def search_images(self):
        query = self.search_input.text()
        if not query:
            QMessageBox.warning(self, "提示", "请输入搜索关键词")
            return
            
        # 清除现有内容
        for i in reversed(range(self.grid_layout.count())): 
            self.grid_layout.itemAt(i).widget().setParent(None)
            
        # 获取新图片
        images = self.api.search_photos(query)
        if not images:
            QMessageBox.information(self, "提示", "未找到匹配的图片")
            return
            
        for i, image in enumerate(images):
            try:
                card = ImageCard()
                card_layout = QVBoxLayout(card)
                card_layout.setSpacing(8)
                
                # 图片预览
                preview_label = QLabel()
                preview_label.setFixedSize(260, 180)
                preview_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
                preview_label.setStyleSheet("""\
                    QLabel {{
                        background-color: #E0E0E0;
                        border-radius: 4px;
                    }}
                """)
                
                # 分辨率信息
                resolution_label = QLabel(f"分辨率: {image['width']} x {image['height']}")
                resolution_label.setStyleSheet("color: #757575; font-size: 12px;")
                
                # 下载缩略图
                thumb_thread = ImageDownloader(
                    image['urls']['small'],
                    os.path.join(self.downloads_folder, f"thumb_{image['id']}.jpg")
                )
                thumb_thread.finished.connect(
                    lambda path, url, label=preview_label: self.update_preview(path, label)
                )
                thumb_thread.start()
                self.image_threads.append(thumb_thread)
                
                # 一键设置按钮
                set_all_btn = MaterialButton("一键设置壁纸和锁屏")
                set_all_btn.clicked.connect(
                    lambda _, img=image:
                    self.set_all_backgrounds(get_highest_resolution_url(img), img['id'])
                )
                
                # 信息标签
                if 'description' in image and image['description']:
                    info_label = QLabel(image['description'][:100] + "..." if len(image['description']) > 100 else image['description'])
                else:
                    info_label = QLabel("无描述")
                info_label.setWordWrap(True)
                info_label.setStyleSheet("color: #757575; font-size: 12px;")
                
                card_layout.addWidget(preview_label)
                card_layout.addWidget(resolution_label)
                card_layout.addWidget(set_all_btn)
                card_layout.addWidget(info_label)
                
                self.grid_layout.addWidget(card, i // 3, i % 3)
                
            except Exception as e:
                logger.error(f"处理图片 {i} 时出错: {str(e)}")
    
    def update_preview(self, image_path, label):
        try:
            pixmap = QPixmap(image_path)
            label.setPixmap(pixmap.scaled(
                label.size(), 
                Qt.AspectRatioMode.KeepAspectRatio,
                Qt.TransformationMode.SmoothTransformation
            ))
        except Exception as e:
            logger.error(f"更新预览图失败: {str(e)}")
    
    def set_all_backgrounds(self, url, image_id):
        """一键设置壁纸和锁屏"""
        logger.info(f"开始下载高分辨率图片: {url}")
        save_path = os.path.join(self.downloads_folder, f"{image_id}.jpg")
        
        # 下载高清图片
        downloader = ImageDownloader(url, save_path)
        downloader.finished.connect(lambda path, _: self._apply_all_backgrounds(path))
        downloader.start()
        self.image_threads.append(downloader)
        
    def _apply_all_backgrounds(self, image_path):
        """应用壁纸和锁屏背景（Windows 11 版本）"""
        success_messages = []
        error_messages = []
        
        # 设置壁纸
        try:
            SPI_SETDESKWALLPAPER = 0x0014
            SPIF_UPDATEINIFILE = 0x01
            SPIF_SENDCHANGE = 0x02
            
            abs_path = os.path.abspath(image_path)
            if ctypes.windll.user32.SystemParametersInfoW(
                SPI_SETDESKWALLPAPER, 0, abs_path, 
                SPIF_UPDATEINIFILE | SPIF_SENDCHANGE
            ):
                success_messages.append("壁纸设置成功")
                logger.info("壁纸设置成功")
            else:
                error_messages.append("壁纸设置失败")
                logger.error("壁纸设置失败")
        except Exception as e:
            error_messages.append(f"壁纸设置错误: {str(e)}")
            logger.error(f"壁纸设置错误: {str(e)}")
        
        # Windows 11 锁屏设置
        try:
            abs_path = os.path.abspath(image_path)
            logger.info(f"开始设置Windows 11锁屏背景: {abs_path}")
            
            # 复制图片到Windows默认锁屏图片目录
            system_drive = os.environ.get('SystemDrive', 'C:')
            assets_path = os.path.join(system_drive, os.sep, 'Windows', 'Web', 'Screen')
            os.makedirs(assets_path, exist_ok=True)
            target_path = os.path.join(assets_path, 'LockScreen.jpg')
            
            # 备份原始锁屏图片
            if os.path.exists(target_path):
                backup_path = os.path.join(assets_path, 'LockScreen_backup.jpg')
                try:
                    if os.path.exists(backup_path):
                        os.remove(backup_path)
                    os.rename(target_path, backup_path)
                    logger.info("原始锁屏图片已备份")
                except Exception as e:
                    logger.error(f"备份原始锁屏图片失败: {str(e)}")
            
            # 复制新图片
            import shutil
            shutil.copy2(abs_path, target_path)
            logger.info("新锁屏图片已复制到系统目录")
            
            # 设置注册表
            try:
                # 方法1：设置personalization CSP
                key_path = r"SOFTWARE\Microsoft\Windows\CurrentVersion\PersonalizationCSP"
                with winreg.CreateKeyEx(winreg.HKEY_LOCAL_MACHINE, key_path, 0, 
                                      winreg.KEY_ALL_ACCESS) as key:
                    winreg.SetValueEx(key, "LockScreenImagePath", 0, winreg.REG_SZ, target_path)
                    winreg.SetValueEx(key, "LockScreenImageStatus", 0, winreg.REG_DWORD, 1)
                logger.info("注册表设置完成（方法1）")
            except Exception as e:
                logger.error(f"方法1失败: {str(e)}")
                
                try:
                    # 方法2：使用默认个性化设置
                    key_path = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Lock Screen\Creative"
                    with winreg.CreateKeyEx(winreg.HKEY_LOCAL_MACHINE, key_path, 0, 
                                          winreg.KEY_ALL_ACCESS) as key:
                        winreg.SetValueEx(key, "LockScreenImagePath", 0, winreg.REG_SZ, target_path)
                        winreg.SetValueEx(key, "HotspotImageFolderPath", 0, winreg.REG_SZ, assets_path)
                        winreg.SetValueEx(key, "CreativeId", 0, winreg.REG_SZ, "")
                    logger.info("注册表设置完成（方法2）")
                except Exception as e:
                    logger.error(f"方法2失败: {str(e)}")
                    
                    # 方法3：使用PowerShell命令
                    try:
                        ps_commands = [
                            # 停止锁屏服务
                            'Stop-Service -Name "LockApp" -Force',
                            # 设置锁屏图片
                            f'Set-ItemProperty -Path "HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\PersonalizationCSP" -Name "LockScreenImagePath" -Value "{target_path}"',
                            f'Set-ItemProperty -Path "HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\PersonalizationCSP" -Name "LockScreenImageStatus" -Value 1',
                            # 重启锁屏服务
                            'Start-Service -Name "LockApp"'
                        ]
                        
                        for cmd in ps_commands:
                            subprocess.run(["powershell", "-Command", cmd], 
                                         capture_output=True, text=True, check=True)
                        logger.info("PowerShell命令执行完成")
                    except Exception as e:
                        logger.error(f"方法3失败: {str(e)}")
                        raise
            
            # 刷新设置
            try:
                # 重启锁屏服务
                subprocess.run(["powershell", "-Command", "Stop-Service 'LockApp' -Force; Start-Service 'LockApp'"],
                             capture_output=True, text=True)
                
                # 刷新个性化设置
                subprocess.run("RUNDLL32.EXE user32.dll,UpdatePerUserSystemParameters ,1 ,True", 
                             shell=True)
            except Exception as e:
                logger.error(f"刷新设置失败: {str(e)}")
            
            success_messages.append("锁屏背景设置成功")
            logger.info("锁屏背景设置成功")
            
        except Exception as e:
            error_messages.append(f"锁屏设置错误: {str(e)}\n请检查管理员权限或手动设置")
            logger.error(f"锁屏设置错误: {str(e)}")
        
        # 显示结果
        if success_messages and not error_messages:
            QMessageBox.information(self, "成功", "\n".join(success_messages) + "\n\n请锁定屏幕查看效果")
        elif error_messages:
            error_text = "\n".join(error_messages)
            if success_messages:
                error_text = "\n".join(success_messages) + "\n\n错误信息:\n" + error_text
            QMessageBox.warning(self, "错误", error_text)
    
    def closeEvent(self, event):
        """程序关闭时清理资源"""
        logger.info("程序正在关闭")
        for thread in self.image_threads:
            thread.quit()
            thread.wait()
        super().closeEvent(event)

def check_admin():
    """检查是否具有管理员权限"""
    try:
        return ctypes.windll.shell32.IsUserAnAdmin()
    except:
        return False

if __name__ == "__main__":
    try:
        logger.info("程序启动")
        
        # 检查管理员权限
        if not check_admin():
            logger.warning("程序未以管理员权限运行，某些功能可能受限")
            ctypes.windll.shell32.ShellExecuteW(
                None, "runas", sys.executable, f'"{sys.argv[0]}"', None, 1
            )
            sys.exit()
        
        # 创建并运行应用
        app = QApplication(sys.argv)
        
        # 设置应用程序样式
        app.setStyle("Fusion")
        
        # 设置应用程序图标
        icon_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "icon.ico")
        if os.path.exists(icon_path):
            app.setWindowIcon(QIcon(icon_path))
        
        # 创建并显示主窗口
        window = WallpaperApp()
        window.show()
        
        # 运行应用程序
        sys.exit(app.exec())
    except Exception as e:
        logger.critical(f"程序发生严重错误: {str(e)}")
        raise