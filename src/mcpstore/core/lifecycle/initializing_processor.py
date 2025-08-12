"""
INITIALIZING状态快速处理器
专门处理INITIALIZING状态的服务，确保快速状态收敛
"""

import asyncio
import logging
from datetime import datetime, timedelta
from typing import Set, Tuple, Optional, List
from mcpstore.core.models.service import ServiceConnectionState

logger = logging.getLogger(__name__)


class InitializingStateProcessor:
    """INITIALIZING状态专用快速处理器"""
    
    def __init__(self, lifecycle_manager):
        self.lifecycle_manager = lifecycle_manager
        self.registry = lifecycle_manager.registry
        
        # 处理状态跟踪
        self.processing_services: Set[Tuple[str, str]] = set()  # (agent_id, service_name)
        self.processor_task: Optional[asyncio.Task] = None
        self.is_running = False
        
        # 配置参数
        self.check_interval = 0.2  # 200ms检查一次
        self.max_concurrent = 15   # 最大并发处理数
        self.timeout_per_service = 3.0  # 每个服务3秒超时
        self.max_processing_time = 30.0  # 单个服务最大处理时间
        
        logger.info("InitializingStateProcessor initialized")
    
    async def start(self):
        """启动INITIALIZING状态快速处理器"""
        if self.is_running:
            logger.warning("InitializingStateProcessor is already running")
            return
            
        self.is_running = True
        try:
            loop = asyncio.get_running_loop()
            self.processor_task = loop.create_task(self._fast_processing_loop())
            self.processor_task.add_done_callback(self._task_done_callback)
            logger.info("InitializingStateProcessor started")
        except Exception as e:
            self.is_running = False
            logger.error(f"Failed to start InitializingStateProcessor: {e}")
            raise
    
    async def stop(self):
        """停止快速处理器"""
        self.is_running = False
        
        if self.processor_task and not self.processor_task.done():
            logger.debug("Cancelling initializing processor task...")
            self.processor_task.cancel()
            try:
                await self.processor_task
            except asyncio.CancelledError:
                logger.debug("Initializing processor task was cancelled")
            except Exception as e:
                logger.error(f"Error during processor task cancellation: {e}")
        
        self.processing_services.clear()
        logger.info("InitializingStateProcessor stopped")
    
    def _task_done_callback(self, task):
        """任务完成回调"""
        if task.exception():
            logger.error(f"InitializingStateProcessor task failed: {task.exception()}")
    
    async def _fast_processing_loop(self):
        """INITIALIZING状态快速处理主循环"""
        logger.info("Starting INITIALIZING fast processing loop")
        
        while self.is_running:
            try:
                # 获取所有INITIALIZING状态的服务
                initializing_services = self._get_initializing_services()
                
                if initializing_services:
                    logger.debug(f"🚀 [FAST_INIT] 发现{len(initializing_services)}个INITIALIZING服务")
                    
                    # 过滤掉正在处理的服务
                    new_services = [
                        (agent_id, service_name) for agent_id, service_name in initializing_services
                        if (agent_id, service_name) not in self.processing_services
                    ]
                    
                    if new_services:
                        logger.debug(f"🚀 [FAST_INIT] 开始处理{len(new_services)}个新的INITIALIZING服务")
                        
                        # 创建处理任务（使用信号量控制并发）
                        semaphore = asyncio.Semaphore(self.max_concurrent)
                        tasks = []
                        
                        for agent_id, service_name in new_services:
                            self.processing_services.add((agent_id, service_name))
                            task = asyncio.create_task(
                                self._process_initializing_service_with_semaphore(
                                    semaphore, agent_id, service_name
                                )
                            )
                            tasks.append(task)
                        
                        # 并发执行，不等待结果（让任务在后台运行）
                        if tasks:
                            asyncio.create_task(asyncio.gather(*tasks, return_exceptions=True))
                
                await asyncio.sleep(self.check_interval)
                
            except asyncio.CancelledError:
                logger.info("INITIALIZING fast processing loop was cancelled")
                break
            except Exception as e:
                logger.error(f"❌ [FAST_INIT] 快速处理器循环异常: {e}")
                await asyncio.sleep(1.0)
        
        logger.info("INITIALIZING fast processing loop ended")
    
    def _get_initializing_services(self) -> List[Tuple[str, str]]:
        """🔧 [REFACTOR] 从Registry获取所有INITIALIZING状态的服务"""
        initializing_services = []

        try:
            # 🔧 [REFACTOR] 从Registry获取所有agent的服务状态
            for agent_id in self.lifecycle_manager.registry.service_states.keys():
                service_names = self.lifecycle_manager.registry.get_all_service_names(agent_id)
                for service_name in service_names:
                    state = self.lifecycle_manager.get_service_state(agent_id, service_name)
                    if state == ServiceConnectionState.INITIALIZING:
                        initializing_services.append((agent_id, service_name))
        except Exception as e:
            logger.error(f"❌ [FAST_INIT] 获取INITIALIZING服务列表失败: {e}")

        return initializing_services
    
    async def _process_initializing_service_with_semaphore(self, semaphore, agent_id: str, service_name: str):
        """带信号量的服务处理"""
        async with semaphore:
            try:
                logger.debug(f"🔧 [FAST_INIT] 开始处理INITIALIZING服务: {service_name}")
                
                # 使用现有的初始连接逻辑，但加上超时
                await asyncio.wait_for(
                    self.lifecycle_manager._attempt_initial_connection(agent_id, service_name),
                    timeout=self.timeout_per_service
                )
                
                logger.debug(f"✅ [FAST_INIT] 服务{service_name}处理完成")
                
            except asyncio.TimeoutError:
                logger.warning(f"⏰ [FAST_INIT] 服务{service_name}初始化超时，标记为DISCONNECTED")
                await self.lifecycle_manager._transition_to_state(
                    agent_id, service_name, ServiceConnectionState.DISCONNECTED
                )
            except Exception as e:
                logger.error(f"❌ [FAST_INIT] 处理服务{service_name}失败: {e}")
                await self.lifecycle_manager._transition_to_state(
                    agent_id, service_name, ServiceConnectionState.DISCONNECTED
                )
            finally:
                # 从处理集合中移除
                self.processing_services.discard((agent_id, service_name))
                logger.debug(f"🔧 [FAST_INIT] 服务{service_name}处理完毕，从处理队列移除")

    async def trigger_immediate_processing(self, agent_id: str, service_name: str):
        """触发立即处理（供add_service调用）"""
        if (agent_id, service_name) not in self.processing_services:
            logger.debug(f"🚀 [FAST_INIT] 触发立即处理: {service_name}")
            self.processing_services.add((agent_id, service_name))
            
            # 创建立即处理任务
            asyncio.create_task(
                self._process_initializing_service_with_semaphore(
                    asyncio.Semaphore(1), agent_id, service_name
                )
            )
