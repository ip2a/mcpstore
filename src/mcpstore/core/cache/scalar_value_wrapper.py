"""
标量值包装器

负责处理 pykv 标量值的包装和解包。
pykv 要求存储的值必须是字典类型，因此需要将标量值包装为 {"value": scalar}。
"""

import logging
from typing import Any

logger = logging.getLogger(__name__)


class ScalarValueWrapper:
    """
    标量值包装器
    
    提供标量值的包装、解包和格式检查功能。
    """
    
    @staticmethod
    def wrap(value: Any) -> dict:
        """
        包装标量值
        
        规则：
        - 如果值已经是字典，直接返回
        - 否则，包装为 {"value": value}
        
        Args:
            value: 要包装的值
            
        Returns:
            包装后的字典
            
        Examples:
            >>> ScalarValueWrapper.wrap("hello")
            {"value": "hello"}
            
            >>> ScalarValueWrapper.wrap(42)
            {"value": 42}
            
            >>> ScalarValueWrapper.wrap(True)
            {"value": True}
            
            >>> ScalarValueWrapper.wrap({"key": "value"})
            {"key": "value"}
        """
        if isinstance(value, dict):
            # 已经是字典，直接返回
            logger.debug(f"[WRAPPER] 值已经是字典，无需包装: {type(value).__name__}")
            return value
        
        # 包装标量值
        wrapped = {"value": value}
        logger.debug(
            f"[WRAPPER] 包装标量值: type={type(value).__name__}, "
            f"wrapped={wrapped}"
        )
        return wrapped
    
    @staticmethod
    def unwrap(wrapped: Any) -> Any:
        """
        解包标量值
        
        规则：
        - 如果值为 None，返回 None
        - 如果值是字典且只包含 "value" 键，返回 value 的值
        - 否则，直接返回原值
        
        Args:
            wrapped: 包装的值
            
        Returns:
            解包后的值
            
        Examples:
            >>> ScalarValueWrapper.unwrap({"value": "hello"})
            "hello"
            
            >>> ScalarValueWrapper.unwrap({"value": 42})
            42
            
            >>> ScalarValueWrapper.unwrap({"key": "value"})
            {"key": "value"}
            
            >>> ScalarValueWrapper.unwrap(None)
            None
        """
        if wrapped is None:
            logger.debug("[WRAPPER] 值为 None，直接返回")
            return None
        
        # 只解包只包含 "value" 键的字典
        if isinstance(wrapped, dict) and "value" in wrapped and len(wrapped) == 1:
            # 解包标量值
            unwrapped = wrapped["value"]
            logger.debug(
                f"[WRAPPER] 解包标量值: type={type(unwrapped).__name__}, "
                f"value={unwrapped}"
            )
            return unwrapped
        
        # 不是包装格式，直接返回
        logger.debug(
            f"[WRAPPER] 值不是包装格式，直接返回: type={type(wrapped).__name__}"
        )
        return wrapped
    
    @staticmethod
    def is_wrapped(value: Any) -> bool:
        """
        检查值是否为包装格式
        
        规则：
        - 如果值是字典且只包含 "value" 键，返回 True
        - 否则，返回 False
        
        Args:
            value: 要检查的值
            
        Returns:
            如果是包装格式返回 True，否则返回 False
            
        Examples:
            >>> ScalarValueWrapper.is_wrapped({"value": "hello"})
            True
            
            >>> ScalarValueWrapper.is_wrapped({"key": "value"})
            False
            
            >>> ScalarValueWrapper.is_wrapped("hello")
            False
            
            >>> ScalarValueWrapper.is_wrapped(None)
            False
        """
        if not isinstance(value, dict):
            return False
        
        # 检查是否只包含 "value" 键
        is_wrapped_format = "value" in value and len(value) == 1
        
        logger.debug(
            f"[WRAPPER] 检查包装格式: is_wrapped={is_wrapped_format}, "
            f"keys={list(value.keys()) if isinstance(value, dict) else None}"
        )
        
        return is_wrapped_format
