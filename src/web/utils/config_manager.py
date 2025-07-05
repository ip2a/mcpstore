"""
配置管理器
提供Web界面的配置管理功能
"""

import json
import streamlit as st
from typing import Dict, List, Optional, Any
from datetime import datetime
import os

class WebConfigManager:
    """Web界面配置管理器"""
    
    def __init__(self):
        self.config_file = "web_config.json"
        self.default_config = {
            "api": {
                "backend_type": "http",
                "base_url": "http://localhost:18611",
                "timeout": 10,
                "retry_count": 3
            },
            "ui": {
                "theme": "light",
                "auto_refresh": False,
                "refresh_interval": 5,
                "items_per_page": 10,
                "show_advanced_options": False
            },
            "presets": {
                "services": [
                    {
                        "name": "mcpstore-wiki",
                        "url": "http://59.110.160.18:21923/mcp",
                        "description": "MCPStore官方Wiki服务",
                        "category": "官方"
                    },
                    {
                        "name": "mcpstore-demo",
                        "url": "http://59.110.160.18:21924/mcp", 
                        "description": "MCPStore演示服务",
                        "category": "演示"
                    }
                ]
            },
            "monitoring": {
                "enable_notifications": True,
                "alert_thresholds": {
                    "service_health": 80,
                    "response_time": 5000
                }
            }
        }
        self.load_config()
    
    def load_config(self) -> Dict:
        """加载配置"""
        try:
            if os.path.exists(self.config_file):
                with open(self.config_file, 'r', encoding='utf-8') as f:
                    config = json.load(f)
                    # 合并默认配置
                    self.config = self._merge_config(self.default_config, config)
            else:
                self.config = self.default_config.copy()
                self.save_config()
        except Exception as e:
            st.warning(f"加载配置失败，使用默认配置: {e}")
            self.config = self.default_config.copy()
        
        return self.config
    
    def save_config(self) -> bool:
        """保存配置"""
        try:
            with open(self.config_file, 'w', encoding='utf-8') as f:
                json.dump(self.config, f, indent=2, ensure_ascii=False)
            return True
        except Exception as e:
            st.error(f"保存配置失败: {e}")
            return False
    
    def _merge_config(self, default: Dict, user: Dict) -> Dict:
        """合并配置"""
        result = default.copy()
        for key, value in user.items():
            if key in result and isinstance(result[key], dict) and isinstance(value, dict):
                result[key] = self._merge_config(result[key], value)
            else:
                result[key] = value
        return result
    
    def get(self, key_path: str, default=None) -> Any:
        """获取配置值"""
        keys = key_path.split('.')
        value = self.config
        
        try:
            for key in keys:
                value = value[key]
            return value
        except (KeyError, TypeError):
            return default
    
    def set(self, key_path: str, value: Any) -> bool:
        """设置配置值"""
        keys = key_path.split('.')
        config = self.config
        
        try:
            for key in keys[:-1]:
                if key not in config:
                    config[key] = {}
                config = config[key]
            
            config[keys[-1]] = value
            return self.save_config()
        except Exception as e:
            st.error(f"设置配置失败: {e}")
            return False
    
    def reset_to_default(self) -> bool:
        """重置为默认配置"""
        self.config = self.default_config.copy()
        return self.save_config()
    
    def export_config(self) -> str:
        """导出配置"""
        return json.dumps(self.config, indent=2, ensure_ascii=False)
    
    def import_config(self, config_str: str) -> bool:
        """导入配置"""
        try:
            imported_config = json.loads(config_str)
            self.config = self._merge_config(self.default_config, imported_config)
            return self.save_config()
        except Exception as e:
            st.error(f"导入配置失败: {e}")
            return False
    
    def get_preset_services(self) -> List[Dict]:
        """获取预设服务"""
        return self.get('presets.services', [])
    
    def add_preset_service(self, service: Dict) -> bool:
        """添加预设服务"""
        presets = self.get_preset_services()
        presets.append(service)
        return self.set('presets.services', presets)
    
    def remove_preset_service(self, service_name: str) -> bool:
        """移除预设服务"""
        presets = self.get_preset_services()
        presets = [s for s in presets if s.get('name') != service_name]
        return self.set('presets.services', presets)

class SessionManager:
    """会话状态管理器"""
    
    @staticmethod
    def init_session_state():
        """初始化会话状态"""
        # 配置管理器
        if 'config_manager' not in st.session_state:
            st.session_state.config_manager = WebConfigManager()
        
        # API客户端配置
        config_manager = st.session_state.config_manager
        
        if 'api_backend_type' not in st.session_state:
            st.session_state.api_backend_type = config_manager.get('api.backend_type', 'http')
        
        if 'api_base_url' not in st.session_state:
            st.session_state.api_base_url = config_manager.get('api.base_url', 'http://localhost:18611')
        
        # UI配置
        if 'ui_theme' not in st.session_state:
            st.session_state.ui_theme = config_manager.get('ui.theme', 'light')
        
        if 'auto_refresh' not in st.session_state:
            st.session_state.auto_refresh = config_manager.get('ui.auto_refresh', False)
        
        if 'refresh_interval' not in st.session_state:
            st.session_state.refresh_interval = config_manager.get('ui.refresh_interval', 5)
        
        # 数据缓存
        if 'data_cache' not in st.session_state:
            st.session_state.data_cache = {}
        
        if 'cache_timestamps' not in st.session_state:
            st.session_state.cache_timestamps = {}
        
        # 操作历史
        if 'operation_history' not in st.session_state:
            st.session_state.operation_history = []
        
        # 通知系统
        if 'notifications' not in st.session_state:
            st.session_state.notifications = []
        
        # 最后刷新时间
        if 'last_refresh' not in st.session_state:
            st.session_state.last_refresh = datetime.now()
    
    @staticmethod
    def get_cached_data(key: str, max_age_seconds: int = 30) -> Optional[Any]:
        """获取缓存数据"""
        if key not in st.session_state.data_cache:
            return None
        
        timestamp = st.session_state.cache_timestamps.get(key)
        if not timestamp:
            return None
        
        age = (datetime.now() - timestamp).total_seconds()
        if age > max_age_seconds:
            # 缓存过期
            del st.session_state.data_cache[key]
            del st.session_state.cache_timestamps[key]
            return None
        
        return st.session_state.data_cache[key]
    
    @staticmethod
    def set_cached_data(key: str, data: Any):
        """设置缓存数据"""
        st.session_state.data_cache[key] = data
        st.session_state.cache_timestamps[key] = datetime.now()
    
    @staticmethod
    def clear_cache():
        """清除所有缓存"""
        st.session_state.data_cache = {}
        st.session_state.cache_timestamps = {}
    
    @staticmethod
    def add_operation_history(operation: str, details: Dict = None):
        """添加操作历史"""
        history_item = {
            'timestamp': datetime.now(),
            'operation': operation,
            'details': details or {}
        }
        
        st.session_state.operation_history.append(history_item)
        
        # 限制历史记录数量
        if len(st.session_state.operation_history) > 100:
            st.session_state.operation_history = st.session_state.operation_history[-100:]
    
    @staticmethod
    def get_operation_history(limit: int = 10) -> List[Dict]:
        """获取操作历史"""
        history = st.session_state.operation_history
        return sorted(history, key=lambda x: x['timestamp'], reverse=True)[:limit]
    
    @staticmethod
    def add_notification(message: str, type: str = "info", auto_dismiss: bool = True):
        """添加通知"""
        notification = {
            'id': len(st.session_state.notifications),
            'message': message,
            'type': type,  # info, success, warning, error
            'timestamp': datetime.now(),
            'auto_dismiss': auto_dismiss,
            'dismissed': False
        }
        
        st.session_state.notifications.append(notification)
    
    @staticmethod
    def get_active_notifications() -> List[Dict]:
        """获取活跃通知"""
        now = datetime.now()
        active_notifications = []
        
        for notification in st.session_state.notifications:
            if notification['dismissed']:
                continue
            
            # 自动消失的通知5秒后消失
            if notification['auto_dismiss']:
                age = (now - notification['timestamp']).total_seconds()
                if age > 5:
                    notification['dismissed'] = True
                    continue
            
            active_notifications.append(notification)
        
        return active_notifications
    
    @staticmethod
    def dismiss_notification(notification_id: int):
        """消除通知"""
        for notification in st.session_state.notifications:
            if notification['id'] == notification_id:
                notification['dismissed'] = True
                break
