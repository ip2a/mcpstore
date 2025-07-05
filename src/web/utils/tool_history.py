"""
工具使用历史记录管理
提供工具使用历史的记录、查询和统计功能
"""

import json
import os
from datetime import datetime
from typing import Dict, List, Optional
import streamlit as st

class ToolHistoryManager:
    """工具使用历史管理器"""
    
    def __init__(self, history_file: str = "tool_history.json"):
        self.history_file = history_file
        self.history_data = self._load_history()
    
    def _load_history(self) -> List[Dict]:
        """加载历史记录"""
        try:
            if os.path.exists(self.history_file):
                with open(self.history_file, 'r', encoding='utf-8') as f:
                    return json.load(f)
        except Exception as e:
            print(f"加载历史记录失败: {e}")
        return []
    
    def _save_history(self):
        """保存历史记录"""
        try:
            with open(self.history_file, 'w', encoding='utf-8') as f:
                json.dump(self.history_data, f, ensure_ascii=False, indent=2)
        except Exception as e:
            print(f"保存历史记录失败: {e}")
    
    def add_record(self, tool_name: str, args: Dict, result: Dict, 
                   success: bool, execution_time: float, agent_id: Optional[str] = None):
        """添加工具使用记录"""
        record = {
            "tool_name": tool_name,
            "agent_id": agent_id,
            "args": args,
            "result": result,
            "success": success,
            "execution_time": execution_time,
            "timestamp": datetime.now().isoformat()
        }
        
        self.history_data.append(record)
        
        # 限制历史记录数量（保留最近1000条）
        if len(self.history_data) > 1000:
            self.history_data = self.history_data[-1000:]
        
        self._save_history()
    
    def get_history(self, limit: Optional[int] = None, 
                   tool_name: Optional[str] = None,
                   agent_id: Optional[str] = None) -> List[Dict]:
        """获取历史记录"""
        filtered_data = self.history_data
        
        # 按工具名过滤
        if tool_name:
            filtered_data = [r for r in filtered_data if r.get('tool_name') == tool_name]
        
        # 按Agent ID过滤
        if agent_id:
            filtered_data = [r for r in filtered_data if r.get('agent_id') == agent_id]
        
        # 按时间倒序排列
        filtered_data.sort(key=lambda x: x.get('timestamp', ''), reverse=True)
        
        # 限制数量
        if limit:
            filtered_data = filtered_data[:limit]
        
        return filtered_data
    
    def get_statistics(self, agent_id: Optional[str] = None) -> Dict:
        """获取使用统计"""
        history = self.get_history(agent_id=agent_id)
        
        if not history:
            return {
                "total_executions": 0,
                "unique_tools": 0,
                "success_rate": 0,
                "avg_execution_time": 0,
                "tool_usage": {},
                "recent_activity": []
            }
        
        # 基本统计
        total_executions = len(history)
        unique_tools = len(set(r['tool_name'] for r in history))
        successful_executions = sum(1 for r in history if r.get('success', False))
        success_rate = (successful_executions / total_executions) * 100 if total_executions > 0 else 0
        
        # 平均执行时间
        execution_times = [r.get('execution_time', 0) for r in history if r.get('execution_time')]
        avg_execution_time = sum(execution_times) / len(execution_times) if execution_times else 0
        
        # 工具使用频率
        tool_usage = {}
        for record in history:
            tool_name = record['tool_name']
            if tool_name not in tool_usage:
                tool_usage[tool_name] = {
                    "count": 0,
                    "success_count": 0,
                    "avg_time": 0
                }
            
            tool_usage[tool_name]["count"] += 1
            if record.get('success', False):
                tool_usage[tool_name]["success_count"] += 1
            
            if record.get('execution_time'):
                current_avg = tool_usage[tool_name]["avg_time"]
                current_count = tool_usage[tool_name]["count"]
                new_time = record['execution_time']
                tool_usage[tool_name]["avg_time"] = (current_avg * (current_count - 1) + new_time) / current_count
        
        # 计算成功率
        for tool_data in tool_usage.values():
            tool_data["success_rate"] = (tool_data["success_count"] / tool_data["count"]) * 100
        
        # 最近活动
        recent_activity = history[:10]  # 最近10条记录
        
        return {
            "total_executions": total_executions,
            "unique_tools": unique_tools,
            "success_rate": success_rate,
            "avg_execution_time": avg_execution_time,
            "tool_usage": tool_usage,
            "recent_activity": recent_activity
        }
    
    def clear_history(self):
        """清空历史记录"""
        self.history_data = []
        self._save_history()

# 全局历史管理器实例
_history_manager = None

def get_history_manager() -> ToolHistoryManager:
    """获取历史管理器实例"""
    global _history_manager
    if _history_manager is None:
        _history_manager = ToolHistoryManager()
    return _history_manager

def record_tool_usage(tool_name: str, args: Dict, result: Dict, 
                     success: bool, execution_time: float, agent_id: Optional[str] = None):
    """记录工具使用"""
    manager = get_history_manager()
    manager.add_record(tool_name, args, result, success, execution_time, agent_id)

def get_tool_history(limit: Optional[int] = None, 
                    tool_name: Optional[str] = None,
                    agent_id: Optional[str] = None) -> List[Dict]:
    """获取工具历史"""
    manager = get_history_manager()
    return manager.get_history(limit, tool_name, agent_id)

def get_tool_statistics(agent_id: Optional[str] = None) -> Dict:
    """获取工具统计"""
    manager = get_history_manager()
    return manager.get_statistics(agent_id)

def clear_tool_history():
    """清空工具历史"""
    manager = get_history_manager()
    manager.clear_history()

# Streamlit集成函数
def show_tool_statistics_ui(agent_id: Optional[str] = None):
    """显示工具统计UI"""
    stats = get_tool_statistics(agent_id)
    
    if stats["total_executions"] == 0:
        st.info("暂无工具使用记录")
        return
    
    # 基本统计
    col1, col2, col3, col4 = st.columns(4)
    
    with col1:
        st.metric("总执行次数", stats["total_executions"])
    
    with col2:
        st.metric("使用过的工具", stats["unique_tools"])
    
    with col3:
        st.metric("成功率", f"{stats['success_rate']:.1f}%")
    
    with col4:
        st.metric("平均执行时间", f"{stats['avg_execution_time']:.2f}s")
    
    # 工具使用排行
    if stats["tool_usage"]:
        st.markdown("#### 🏆 工具使用排行")
        
        # 按使用次数排序
        sorted_tools = sorted(stats["tool_usage"].items(), 
                            key=lambda x: x[1]["count"], reverse=True)
        
        for i, (tool_name, tool_data) in enumerate(sorted_tools[:10]):
            with st.expander(f"{i+1}. {tool_name} ({tool_data['count']} 次)"):
                col1, col2, col3 = st.columns(3)
                
                with col1:
                    st.metric("使用次数", tool_data["count"])
                
                with col2:
                    st.metric("成功率", f"{tool_data['success_rate']:.1f}%")
                
                with col3:
                    st.metric("平均时间", f"{tool_data['avg_time']:.2f}s")
    
    # 最近活动
    if stats["recent_activity"]:
        st.markdown("#### 📝 最近活动")
        
        for record in stats["recent_activity"][:5]:
            with st.container():
                col1, col2, col3, col4 = st.columns([2, 1, 1, 2])
                
                with col1:
                    st.write(f"**{record['tool_name']}**")
                
                with col2:
                    status_icon = "✅" if record.get('success', False) else "❌"
                    st.write(status_icon)
                
                with col3:
                    if record.get('execution_time'):
                        st.write(f"{record['execution_time']:.2f}s")
                
                with col4:
                    timestamp = record.get('timestamp', '')
                    if timestamp:
                        try:
                            dt = datetime.fromisoformat(timestamp.replace('Z', '+00:00'))
                            st.write(dt.strftime("%m-%d %H:%M"))
                        except:
                            st.write(timestamp[:16])

def show_tool_history_ui(limit: int = 50, agent_id: Optional[str] = None):
    """显示工具历史UI"""
    history = get_tool_history(limit=limit, agent_id=agent_id)
    
    if not history:
        st.info("暂无工具使用历史")
        return
    
    st.markdown(f"#### 📋 工具使用历史 (最近 {len(history)} 条)")
    
    for i, record in enumerate(history):
        with st.expander(f"{i+1}. {record['tool_name']} - {record.get('timestamp', '')[:16]}"):
            col1, col2 = st.columns(2)
            
            with col1:
                st.markdown("**基本信息**:")
                st.write(f"- 工具名称: {record['tool_name']}")
                st.write(f"- Agent ID: {record.get('agent_id', 'Store级别')}")
                st.write(f"- 执行状态: {'✅ 成功' if record.get('success', False) else '❌ 失败'}")
                if record.get('execution_time'):
                    st.write(f"- 执行时间: {record['execution_time']:.2f}秒")
            
            with col2:
                st.markdown("**参数和结果**:")
                if record.get('args'):
                    st.code(json.dumps(record['args'], ensure_ascii=False, indent=2), language='json')
                
                if record.get('result'):
                    result_str = json.dumps(record['result'], ensure_ascii=False, indent=2)
                    if len(result_str) > 500:
                        result_str = result_str[:500] + "..."
                    st.code(result_str, language='json')
