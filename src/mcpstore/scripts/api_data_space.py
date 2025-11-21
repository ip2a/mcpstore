"""
MCPStore API - Data Space Management Routes
Contains data space and workspace management related API endpoints
"""

import logging
import os
from typing import Dict, Any, List, Optional

from fastapi import APIRouter, HTTPException, Depends
from mcpstore.core.models.common import APIResponse

from .api_decorators import handle_exceptions, get_store

# Create data space router
data_space_router = APIRouter()

logger = logging.getLogger(__name__)

@data_space_router.get("/data_space/info", response_model=APIResponse)
@handle_exceptions
async def get_data_space_info():
    """获取当前数据空间信息"""
    try:
        store = get_store()
        info = store.get_data_space_info()
        
        return APIResponse(
            success=True,
            data=info,
            message="Data space information retrieved successfully"
        )
    except Exception as e:
        logger.error(f"Failed to get data space info: {e}")
        raise HTTPException(status_code=500, detail=f"Failed to get data space info: {str(e)}")

@data_space_router.get("/workspace/list", response_model=APIResponse)
@handle_exceptions
async def list_workspaces():
    """列出所有可用的工作空间"""
    try:
        store = get_store()
        # 获取当前数据空间目录的父目录
        if store.is_using_data_space():
            current_workspace = store.get_workspace_dir()
            parent_dir = os.path.dirname(current_workspace)
            
            # 查找所有包含 mcp.json 的工作空间
            workspaces = []
            if os.path.exists(parent_dir):
                for item in os.listdir(parent_dir):
                    item_path = os.path.join(parent_dir, item)
                    if os.path.isdir(item_path):
                        mcp_file = os.path.join(item_path, "mcp.json")
                        if os.path.exists(mcp_file):
                            workspaces.append({
                                "name": item,
                                "path": item_path,
                                "mcp_config_path": mcp_file,
                                "is_current": item_path == current_workspace
                            })
            
            return APIResponse(
                success=True,
                data={
                    "workspaces": workspaces,
                    "current_workspace": current_workspace if store.is_using_data_space() else None
                },
                message=f"Found {len(workspaces)} workspaces"
            )
        else:
            return APIResponse(
                success=True,
                data={
                    "workspaces": [],
                    "current_workspace": None,
                    "using_default": True
                },
                message="Using default configuration (no data space)"
            )
    except Exception as e:
        logger.error(f"Failed to list workspaces: {e}")
        raise HTTPException(status_code=500, detail=f"Failed to list workspaces: {str(e)}")

@data_space_router.post("/workspace/switch", response_model=APIResponse)
@handle_exceptions
async def switch_workspace(payload: Dict[str, Any]):
    """切换到指定的工作空间
    
    Expected payload:
    {
        "workspace_path": "/path/to/workspace",  # 可选，如果不提供则切换到默认配置
        "mcp_config_file": "/path/to/mcp.json"  # 可选，指定配置文件路径
    }
    """
    try:
        workspace_path = payload.get("workspace_path")
        mcp_config_file = payload.get("mcp_config_file")
        
        if not workspace_path and not mcp_config_file:
            # 切换到默认配置
            from mcpstore import MCPStore
            new_store = MCPStore.setup_store(debug=False)
            
            return APIResponse(
                success=True,
                data={
                    "switched_to_default": True,
                    "store_info": {
                        "is_using_data_space": new_store.is_using_data_space(),
                        "workspace_dir": new_store.get_workspace_dir() if new_store.is_using_data_space() else None
                    }
                },
                message="Switched to default configuration successfully"
            )
        
        # 切换到指定工作空间
        from mcpstore import MCPStore
        if mcp_config_file:
            new_store = MCPStore.setup_store(mcp_config_file=mcp_config_file, debug=False)
        else:
            new_store = MCPStore._setup_with_data_space(workspace_path)
        
        # 更新全局 store 实例
        from .api_app import set_global_store
        set_global_store(new_store)
        
        return APIResponse(
            success=True,
            data={
                "switched_to_default": False,
                "workspace_path": workspace_path,
                "mcp_config_file": mcp_config_file,
                "store_info": {
                    "is_using_data_space": new_store.is_using_data_space(),
                    "workspace_dir": new_store.get_workspace_dir() if new_store.is_using_data_space() else None
                }
            },
            message="Workspace switched successfully"
        )
    except Exception as e:
        logger.error(f"Failed to switch workspace: {e}")
        raise HTTPException(status_code=500, detail=f"Failed to switch workspace: {str(e)}")

@data_space_router.post("/workspace/create", response_model=APIResponse)
@handle_exceptions
async def create_workspace(payload: Dict[str, Any]):
    """创建新的工作空间
    
    Expected payload:
    {
        "name": "workspace_name",  # 工作空间名称
        "path": "/path/to/workspace",  # 可选，默认在父目录下创建
        "template": "default"  # 可选，模板类型
    }
    """
    try:
        name = payload.get("name")
        base_path = payload.get("path")
        template = payload.get("template", "default")
        
        if not name:
            raise HTTPException(status_code=400, detail="Workspace name is required")
        
        # 确定工作空间路径
        if base_path:
            workspace_path = os.path.join(base_path, name)
        else:
            # 使用当前数据空间的父目录
            store = get_store()
            if store.is_using_data_space():
                current_workspace = store.get_workspace_dir()
                parent_dir = os.path.dirname(current_workspace)
                workspace_path = os.path.join(parent_dir, name)
            else:
                # 如果当前没有使用数据空间，创建在默认位置
                from mcpstore.config.config import LoggingConfig
                config = LoggingConfig()
                parent_dir = os.path.dirname(config.get_default_mcp_path())
                workspace_path = os.path.join(parent_dir, name)
        
        # 创建目录
        os.makedirs(workspace_path, exist_ok=True)
        
        # 创建默认的 mcp.json
        mcp_file = os.path.join(workspace_path, "mcp.json")
        if template == "default":
            default_config = {
                "mcpServers": {},
                "workspace": {
                    "name": name,
                    "created_at": "2025-01-01T00:00:00Z",
                    "description": f"Workspace {name}"
                }
            }
            
            import json
            with open(mcp_file, 'w', encoding='utf-8') as f:
                json.dump(default_config, f, indent=2, ensure_ascii=False)
        
        return APIResponse(
            success=True,
            data={
                "workspace_path": workspace_path,
                "mcp_config_path": mcp_file,
                "name": name,
                "template": template
            },
            message=f"Workspace '{name}' created successfully"
        )
    except Exception as e:
        logger.error(f"Failed to create workspace: {e}")
        raise HTTPException(status_code=500, detail=f"Failed to create workspace: {str(e)}")

@data_space_router.get("/workspace/current", response_model=APIResponse)
@handle_exceptions
async def get_current_workspace():
    """获取当前工作空间信息"""
    try:
        store = get_store()
        
        if store.is_using_data_space():
            workspace_dir = store.get_workspace_dir()
            mcp_config_path = store.config.json_path if hasattr(store.config, 'json_path') else None
            
            # 读取工作空间配置
            workspace_config = {}
            if mcp_config_path and os.path.exists(mcp_config_path):
                try:
                    import json
                    with open(mcp_config_path, 'r', encoding='utf-8') as f:
                        config_data = json.load(f)
                        workspace_config = config_data.get("workspace", {})
                except:
                    pass
            
            return APIResponse(
                success=True,
                data={
                    "is_using_data_space": True,
                    "workspace_dir": workspace_dir,
                    "mcp_config_path": mcp_config_path,
                    "workspace_config": workspace_config
                },
                message="Current workspace information retrieved"
            )
        else:
            return APIResponse(
                success=True,
                data={
                    "is_using_data_space": False,
                    "workspace_dir": None,
                    "mcp_config_path": getattr(store.config, 'json_path', None),
                    "workspace_config": {}
                },
                message="Using default configuration (no workspace)"
            )
    except Exception as e:
        logger.error(f"Failed to get current workspace: {e}")
        raise HTTPException(status_code=500, detail=f"Failed to get current workspace: {str(e)}")

@data_space_router.delete("/workspace/{workspace_name}", response_model=APIResponse)
@handle_exceptions
async def delete_workspace(workspace_name: str):
    """删除指定的工作空间（危险操作）"""
    try:
        store = get_store()
        
        # 获取工作空间路径
        if store.is_using_data_space():
            current_workspace = store.get_workspace_dir()
            parent_dir = os.path.dirname(current_workspace)
            workspace_path = os.path.join(parent_dir, workspace_name)
        else:
            from mcpstore.config.config import LoggingConfig
            config = LoggingConfig()
            parent_dir = os.path.dirname(config.get_default_mcp_path())
            workspace_path = os.path.join(parent_dir, workspace_name)
        
        # 安全检查
        if not os.path.exists(workspace_path):
            raise HTTPException(status_code=404, detail=f"Workspace '{workspace_name}' not found")
        
        if workspace_path == current_workspace:
            raise HTTPException(status_code=400, detail="Cannot delete the currently active workspace")
        
        # 确认这是一个工作空间目录（包含 mcp.json）
        mcp_file = os.path.join(workspace_path, "mcp.json")
        if not os.path.exists(mcp_file):
            raise HTTPException(status_code=400, detail=f"Directory '{workspace_name}' is not a valid workspace")
        
        # 删除工作空间（实际上只是移动到回收站）
        import shutil
        import time
        trash_path = f"{workspace_path}_deleted_{int(time.time())}"
        shutil.move(workspace_path, trash_path)
        
        return APIResponse(
            success=True,
            data={
                "workspace_name": workspace_name,
                "original_path": workspace_path,
                "moved_to": trash_path
            },
            message=f"Workspace '{workspace_name}' moved to trash. To permanently delete, manually remove: {trash_path}"
        )
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Failed to delete workspace: {e}")
        raise HTTPException(status_code=500, detail=f"Failed to delete workspace: {str(e)}")