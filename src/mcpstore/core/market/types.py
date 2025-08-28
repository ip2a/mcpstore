"""
Market data types and models
市场数据类型和模型定义
"""

from typing import Dict, List, Optional, Any, Union
from dataclasses import dataclass, field
from datetime import datetime


@dataclass
class MarketServerRepository:
    """市场服务代码仓库信息"""
    type: str                    # git, npm, pypi等
    url: str                     # 仓库URL


@dataclass  
class MarketServerAuthor:
    """市场服务作者信息"""
    name: str                    # 作者名称
    email: Optional[str] = None  # 作者邮箱
    url: Optional[str] = None    # 作者主页


@dataclass
class MarketInstallation:
    """市场服务安装配置"""
    type: str                             # 安装类型: npm, uvx, pip, docker等
    command: str                          # 安装命令: npx, uvx, pip等
    args: List[str] = field(default_factory=list)           # 命令参数
    env: Dict[str, str] = field(default_factory=dict)       # 环境变量
    working_dir: Optional[str] = None     # 工作目录


@dataclass
class MarketServerExample:
    """市场服务使用示例"""
    title: str                   # 示例标题
    description: str             # 示例描述
    prompt: str                  # 示例提示词


@dataclass
class MarketServerTool:
    """市场服务工具信息"""
    name: str                           # 工具名称
    description: str                    # 工具描述
    input_schema: Dict[str, Any] = field(default_factory=dict)  # 输入模式


@dataclass
class MarketServerInfo:
    """市场服务完整信息"""
    name: str                                              # 服务名称
    display_name: str                                      # 显示名称
    description: str                                       # 服务描述
    repository: MarketServerRepository                     # 代码仓库
    homepage: str                                          # 主页URL
    author: MarketServerAuthor                             # 作者信息
    license: str                                           # 许可证
    categories: List[str] = field(default_factory=list)   # 分类
    tags: List[str] = field(default_factory=list)         # 标签
    examples: List[MarketServerExample] = field(default_factory=list)  # 使用示例
    installations: Dict[str, MarketInstallation] = field(default_factory=dict)  # 安装方式
    tools: List[MarketServerTool] = field(default_factory=list)  # 工具列表
    is_official: bool = False                              # 是否官方服务
    created_at: Optional[datetime] = None                  # 创建时间
    updated_at: Optional[datetime] = None                  # 更新时间
    

@dataclass
class MarketSearchFilter:
    """市场搜索过滤器"""
    query: Optional[str] = None                    # 搜索关键词
    categories: List[str] = field(default_factory=list)  # 分类过滤
    tags: List[str] = field(default_factory=list)        # 标签过滤
    is_official: Optional[bool] = None             # 是否只显示官方
    limit: Optional[int] = None                    # 结果数量限制


@dataclass
class MCPStoreServiceConfig:
    """MCPStore服务配置"""
    name: str                                      # 服务名称
    command: str                                   # 执行命令
    args: List[str] = field(default_factory=list) # 命令参数
    env: Dict[str, str] = field(default_factory=dict)     # 环境变量
    working_dir: Optional[str] = None              # 工作目录
    transport: Optional[str] = None                # 传输类型
    url: Optional[str] = None                      # 服务URL（用于HTTP传输）
    
    # 市场元数据
    market_source: str = "manual"                  # 来源标识
    market_name: Optional[str] = None              # 原始市场名称
    market_version: Optional[str] = None           # 市场版本


@dataclass
class MarketInstallResult:
    """市场安装结果"""
    success: bool                                  # 是否成功
    service_config: Optional[MCPStoreServiceConfig] = None  # 生成的服务配置
    error_message: Optional[str] = None            # 错误信息
    warnings: List[str] = field(default_factory=list)      # 警告信息
    market_info: Optional[MarketServerInfo] = None         # 市场服务信息
