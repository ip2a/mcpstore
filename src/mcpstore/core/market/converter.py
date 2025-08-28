"""
Market configuration converter
市场配置转换器 - 将市场服务配置转换为MCPStore格式
"""

import logging
from typing import Dict, Any, Optional, List
from .types import MarketServerInfo, MarketInstallation, MCPStoreServiceConfig, MarketInstallResult

logger = logging.getLogger(__name__)


class MarketConfigConverter:
    """市场配置转换器"""
    
    # 安装方式优先级（从高到低）
    INSTALLATION_PRIORITY = [
        "npm",        # Node.js包管理器，最常用
        "uvx",        # Python包管理器，现代化
        "pip",        # Python传统包管理器
        "python",     # Python直接执行
        "docker",     # Docker容器
        "cargo",      # Rust包管理器
        "go",         # Go语言
        "custom"      # 自定义安装
    ]
    
    def __init__(self):
        self.logger = logger
        
    def convert_market_to_mcpstore(self, 
                                  market_info: MarketServerInfo, 
                                  user_env: Optional[Dict[str, str]] = None,
                                  preferred_installation: Optional[str] = None) -> MarketInstallResult:
        """
        将市场服务配置转换为MCPStore服务配置
        
        Args:
            market_info: 市场服务信息
            user_env: 用户提供的环境变量
            preferred_installation: 优先使用的安装方式
            
        Returns:
            MarketInstallResult: 转换结果
        """
        
        try:
            # 1. 选择安装方式
            installation = self._select_installation(market_info.installations, preferred_installation)
            if not installation:
                return MarketInstallResult(
                    success=False,
                    error_message=f"No suitable installation method found for {market_info.name}",
                    market_info=market_info
                )
            
            # 2. 验证和转换基础配置
            if not installation.command or installation.command.strip() == "":
                return MarketInstallResult(
                    success=False,
                    error_message=f"Empty command in installation configuration for {market_info.name}",
                    market_info=market_info
                )
            
            service_config = MCPStoreServiceConfig(
                name=market_info.name,
                command=installation.command.strip(),
                args=installation.args.copy() if installation.args else [],
                working_dir=installation.working_dir,
                market_source="market",
                market_name=market_info.name
            )
            
            # 3. 处理环境变量
            warnings = []
            final_env = {}
            
            # 从安装配置中获取环境变量模板
            if installation.env:
                final_env.update(installation.env)
            
            # 应用用户提供的环境变量
            if user_env:
                for key, value in user_env.items():
                    final_env[key] = value
            
            # 检查必需的环境变量
            missing_env = self._check_required_env(final_env)
            if missing_env:
                warnings.extend([
                    f"环境变量 '{var}' 未设置，服务可能无法正常工作" 
                    for var in missing_env
                ])
            
            service_config.env = final_env
            
            # 4. 自动检测传输类型
            transport = self._detect_transport_type(installation)
            if transport:
                service_config.transport = transport
            
            self.logger.info(f"Successfully converted market service {market_info.name} using {installation.type} installation")
            
            return MarketInstallResult(
                success=True,
                service_config=service_config,
                warnings=warnings,
                market_info=market_info
            )
            
        except Exception as e:
            self.logger.error(f"Failed to convert market service {market_info.name}: {e}")
            return MarketInstallResult(
                success=False,
                error_message=f"Conversion failed: {str(e)}",
                market_info=market_info
            )
    
    def _select_installation(self, 
                           installations: Dict[str, MarketInstallation], 
                           preferred: Optional[str] = None) -> Optional[MarketInstallation]:
        """
        选择最佳的安装方式
        
        Args:
            installations: 可用的安装方式
            preferred: 用户优先选择的安装方式
            
        Returns:
            MarketInstallation: 选中的安装配置
        """
        
        if not installations:
            return None
        
        # 如果用户指定了优先安装方式且可用，则使用
        if preferred and preferred in installations:
            self.logger.debug(f"Using user preferred installation: {preferred}")
            return installations[preferred]
        
        # 按优先级顺序选择
        for installation_type in self.INSTALLATION_PRIORITY:
            if installation_type in installations:
                self.logger.debug(f"Selected installation type: {installation_type}")
                return installations[installation_type]
        
        # 如果没有匹配的优先级，选择第一个可用的
        first_available = next(iter(installations.values()))
        self.logger.debug(f"Using first available installation: {list(installations.keys())[0]}")
        return first_available
    
    def _check_required_env(self, env_vars: Dict[str, str]) -> List[str]:
        """
        检查必需但未设置的环境变量
        
        Args:
            env_vars: 当前环境变量
            
        Returns:
            List[str]: 缺失的环境变量列表
        """
        
        missing = []
        
        for key, value in env_vars.items():
            # 检查是否为占位符格式 ${VAR_NAME} 或环境变量引用
            if isinstance(value, str):
                if value.startswith("${") and value.endswith("}"):
                    # 这是一个未解析的占位符，提取变量名
                    placeholder_var = value[2:-1]  # 去掉${}
                    missing.append(placeholder_var)
                elif not value or value.strip() == "":
                    # 空值
                    missing.append(key)
        
        return missing
    
    def _detect_transport_type(self, installation: MarketInstallation) -> Optional[str]:
        """
        根据安装配置自动检测传输类型
        
        Args:
            installation: 安装配置
            
        Returns:
            Optional[str]: 检测到的传输类型
        """
        
        # 大多数MCP服务使用stdio传输
        if installation.type in ["npm", "uvx", "pip", "python"]:
            return "stdio"
        
        # Docker通常使用HTTP
        if installation.type == "docker":
            return "http"
        
        # 默认使用stdio
        return "stdio"
    
    def get_installation_info(self, market_info: MarketServerInfo) -> Dict[str, Any]:
        """
        获取服务的安装信息摘要
        
        Args:
            market_info: 市场服务信息
            
        Returns:
            Dict[str, Any]: 安装信息摘要
        """
        
        summary = {
            "name": market_info.name,
            "display_name": market_info.display_name,
            "description": market_info.description,
            "categories": market_info.categories,
            "tags": market_info.tags,
            "is_official": market_info.is_official,
            "available_installations": list(market_info.installations.keys()),
            "recommended_installation": None,
            "required_env_vars": []
        }
        
        # 确定推荐的安装方式
        recommended = self._select_installation(market_info.installations)
        if recommended:
            summary["recommended_installation"] = {
                "type": next(k for k, v in market_info.installations.items() if v == recommended),
                "command": recommended.command,
                "args": recommended.args
            }
            
            # 提取必需的环境变量
            if recommended.env:
                for key, value in recommended.env.items():
                    if isinstance(value, str) and value.startswith("${") and value.endswith("}"):
                        summary["required_env_vars"].append(key)
        
        return summary
