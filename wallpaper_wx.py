import wx
import os
import sys
import ctypes
import requests
import logging
import subprocess
import winreg
from datetime import datetime
from threading import Thread
from PIL import Image

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

class ImageDownloader(Thread):
    def __init__(self, url, save_path, callback):
        super().__init__()
        self.url = url
        self.save_path = save_path
        self.callback = callback
        
    def run(self):
        try:
            logger.info(f"开始下载图片: {self.url}")
            response = requests.get(self.url)
            if response.status_code == 200:
                with open(self.save_path, 'wb') as f:
                    f.write(response.content)
                logger.info(f"图片下载成功: {self.save_path}")
                wx.CallAfter(self.callback, self.save_path)
            else:
                logger.error(f"图片下载失败: HTTP {response.status_code}")
        except Exception as e:
            logger.error(f"下载过程出错: {str(e)}")

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

class ImagePanel(wx.Panel):
    def __init__(self, parent, image_data, downloads_folder):
        super().__init__(parent, style=wx.BORDER_THEME)
        self.image_data = image_data
        self.downloads_folder = downloads_folder
        self.SetBackgroundColour(wx.Colour(245, 245, 245))
        
        # 创建布局
        sizer = wx.BoxSizer(wx.VERTICAL)
        
        # 图片预览容器
        preview_container = wx.Panel(self)
        preview_container.SetBackgroundColour(wx.Colour(224, 224, 224))
        preview_sizer = wx.BoxSizer(wx.VERTICAL)
        
        # 图片预览
        self.image_preview = wx.StaticBitmap(preview_container)
        preview_sizer.Add(self.image_preview, 1, wx.EXPAND | wx.ALL, 5)
        preview_container.SetSizer(preview_sizer)
        
        # 设置预览容器的最小大小
        preview_container.SetMinSize((260, 180))
        sizer.Add(preview_container, 1, wx.EXPAND | wx.ALL, 5)
        
        # 分辨率信息
        resolution_text = f"分辨率: {image_data['width']} x {image_data['height']}"
        resolution_label = wx.StaticText(self, label=resolution_text)
        sizer.Add(resolution_label, 0, wx.ALL | wx.EXPAND, 5)
        
        # 设置按钮
        set_btn = wx.Button(self, label="设置为壁纸和锁屏")
        set_btn.Bind(wx.EVT_BUTTON, self.on_set_background)
        sizer.Add(set_btn, 0, wx.ALL | wx.EXPAND, 5)
        
        # 描述信息
        desc = image_data.get('description', '') or image_data.get('alt_description', '') or '无描述'
        if desc and len(desc) > 100:
            desc = desc[:100] + "..."
        desc_label = wx.StaticText(self, label=desc)
        desc_label.Wrap(250)
        sizer.Add(desc_label, 0, wx.ALL | wx.EXPAND, 5)
        
        self.SetSizer(sizer)
        
        # 下载缩略图
        thumb_url = image_data['urls']['small']
        thumb_path = os.path.join(downloads_folder, f"thumb_{image_data['id']}.jpg")
        self.downloader = ImageDownloader(thumb_url, thumb_path, self.on_thumb_downloaded)
        self.downloader.start()
    
    def on_thumb_downloaded(self, path):
        try:
            image = wx.Image(path, wx.BITMAP_TYPE_ANY)
            # 获取容器大小
            container_size = self.image_preview.GetParent().GetSize()
            # 保持宽高比缩放
            image = image.Scale(
                container_size.width - 10,  # 减去边距
                container_size.height - 10,
                wx.IMAGE_QUALITY_HIGH
            )
            self.image_preview.SetBitmap(wx.Bitmap(image))
            self.Layout()
        except Exception as e:
            logger.error(f"加载预览图失败: {str(e)}")
    
    def on_set_background(self, event):
        url = get_highest_resolution_url(self.image_data)
        save_path = os.path.join(self.downloads_folder, f"{self.image_data['id']}.jpg")
        downloader = ImageDownloader(url, save_path, self.apply_background)
        downloader.start()
    
    def apply_background(self, image_path):
        try:
            # 设置壁纸
            SPI_SETDESKWALLPAPER = 0x0014
            ctypes.windll.user32.SystemParametersInfoW(
                SPI_SETDESKWALLPAPER, 0, image_path, 3
            )
            
            # 设置锁屏
            system_drive = os.environ.get('SystemDrive', 'C:')
            assets_path = os.path.join(system_drive, os.sep, 'Windows', 'Web', 'Screen')
            os.makedirs(assets_path, exist_ok=True)
            target_path = os.path.join(assets_path, 'LockScreen.jpg')
            
            import shutil
            shutil.copy2(image_path, target_path)
            
            # 设置注册表
            key_path = r"SOFTWARE\Microsoft\Windows\CurrentVersion\PersonalizationCSP"
            try:
                with winreg.CreateKeyEx(winreg.HKEY_LOCAL_MACHINE, key_path, 0, 
                                      winreg.KEY_ALL_ACCESS) as key:
                    winreg.SetValueEx(key, "LockScreenImagePath", 0, winreg.REG_SZ, target_path)
                    winreg.SetValueEx(key, "LockScreenImageStatus", 0, winreg.REG_DWORD, 1)
            except Exception:
                subprocess.run(["powershell", "-Command", 
                              f'Set-ItemProperty -Path "HKLM:\\{key_path}" ' +
                              f'-Name "LockScreenImagePath" -Value "{target_path}"'])
            
            wx.MessageBox("壁纸和锁屏设置成功！", "成功")
        except Exception as e:
            logger.error(f"设置背景失败: {str(e)}")
            wx.MessageBox(f"设置失败: {str(e)}", "错误", wx.ICON_ERROR)

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

class WallpaperFrame(wx.Frame):
    def __init__(self):
        super().__init__(None, title="壁纸设置工具", size=(900, 600))
        self.api = UnsplashAPI("PtWrZwThqF0NYvw_XDTKwc4YOrRNWevrZQYfvJ02nLE")
        self.downloads_folder = os.path.join(os.path.expanduser("~"), "Pictures", "Wallpapers")
        os.makedirs(self.downloads_folder, exist_ok=True)
        
        # 创建主面板
        panel = wx.Panel(self)
        main_sizer = wx.BoxSizer(wx.VERTICAL)
        
        # 搜索区域
        search_sizer = wx.BoxSizer(wx.HORIZONTAL)
        self.search_input = wx.TextCtrl(panel, style=wx.TE_PROCESS_ENTER)
        self.search_input.Bind(wx.EVT_TEXT_ENTER, self.on_search)
        search_button = wx.Button(panel, label="搜索")
        search_button.Bind(wx.EVT_BUTTON, self.on_search)
        
        search_sizer.Add(self.search_input, 1, wx.ALL | wx.EXPAND, 5)
        search_sizer.Add(search_button, 0, wx.ALL, 5)
        main_sizer.Add(search_sizer, 0, wx.ALL | wx.EXPAND, 5)
        
        # 图片展示区域
        self.scroll = wx.ScrolledWindow(panel)
        self.scroll.SetScrollRate(0, 20)
        
        # 使用 wx.BoxSizer 作为滚动窗口的主布局
        scroll_sizer = wx.BoxSizer(wx.VERTICAL)
        self.grid_sizer = wx.FlexGridSizer(0, 3, 10, 10)
        
        # 设置 grid_sizer 可以在水平方向上增长
        self.grid_sizer.AddGrowableCol(0, 1)
        self.grid_sizer.AddGrowableCol(1, 1)
        self.grid_sizer.AddGrowableCol(2, 1)
        
        scroll_sizer.Add(self.grid_sizer, 1, wx.EXPAND | wx.ALL, 5)
        self.scroll.SetSizer(scroll_sizer)
        
        main_sizer.Add(self.scroll, 1, wx.EXPAND)
        panel.SetSizer(main_sizer)
        
        # 设置图标
        icon_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "icon.ico")
        if os.path.exists(icon_path):
            icon = wx.Icon(icon_path)
            self.SetIcon(icon)
    
    def on_search(self, event):
        query = self.search_input.GetValue()
        if not query:
            wx.MessageBox("请输入搜索关键词", "提示")
            return
        
        # 清除现有内容
        self.grid_sizer.Clear(True)
        
        # 获取新图片
        images = self.api.search_photos(query)
        if not images:
            wx.MessageBox("未找到匹配的图片", "提示")
            return
        
        # 显示图片
        for image in images:
            panel = ImagePanel(self.scroll, image, self.downloads_folder)
            self.grid_sizer.Add(panel, 0, wx.ALL, 5)
        
        self.grid_sizer.Layout()
        self.scroll.FitInside()

def check_admin():
    try:
        return ctypes.windll.shell32.IsUserAnAdmin()
    except:
        return False

def main():
    if not check_admin():
        logger.warning("程序未以管理员权限运行，某些功能可能受限")
        ctypes.windll.shell32.ShellExecuteW(
            None, "runas", sys.executable, f'"{sys.argv[0]}"', None, 1
        )
        return
    
    app = wx.App()
    frame = WallpaperFrame()
    frame.Show()
    app.MainLoop()

if __name__ == "__main__":
    main()
