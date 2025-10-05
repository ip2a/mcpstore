<!-- 顶部栏 -->
<template>
  <div class="layout-top-bar" :class="[tabStyle]">
    <div class="menu">
      <div class="left" style="display: flex">
        <!-- 系统信息  -->
        <div class="top-header" @click="toHome" v-if="isTopMenu">
          <ArtLogo class="logo" />
          <p v-if="width >= 1400">{{ AppConfig.systemInfo.name }}</p>
        </div>

        <ArtLogo class="logo2" @click="toHome" />

        <!-- 菜单按钮 -->
        <div class="btn-box" v-if="isLeftMenu && shouldShowMenuButton">
          <div class="btn menu-btn">
            <i class="iconfont-sys" @click="visibleMenu">&#xe6ba;</i>
          </div>
        </div>
        <!-- 刷新按钮 -->
        <div class="btn-box" v-if="shouldShowRefreshButton">
          <div class="btn refresh-btn" :style="{ marginLeft: !isLeftMenu ? '10px' : '0' }">
            <i class="iconfont-sys" @click="reload()"> &#xe6b3; </i>
          </div>
        </div>

        <!-- 快速入口 -->
        <ArtFastEnter v-if="shouldShowFastEnter && width >= headerBarFastEnterMinWidth" />

        <!-- 面包屑 -->
        <ArtBreadcrumb
          v-if="(shouldShowBreadcrumb && isLeftMenu) || (shouldShowBreadcrumb && isDualMenu)"
        />

        <!-- 顶部菜单 -->
        <ArtHorizontalMenu v-if="isTopMenu" :list="menuList" />

        <!-- 混合菜单-顶部 -->
        <ArtMixedMenu v-if="isTopLeftMenu" :list="menuList" />
      </div>

      <div class="right">
        <!-- 搜索 -->
        <div class="search-wrap" v-if="shouldShowGlobalSearch">
          <div class="search-input" @click="openSearchDialog">
            <div class="left">
              <i class="iconfont-sys">&#xe710;</i>
              <span>{{ $t('topBar.search.title') }}</span>
            </div>
            <div class="search-keydown">
              <i class="iconfont-sys" v-if="isWindows">&#xeeac;</i>
              <i class="iconfont-sys" v-else>&#xe9ab;</i>
              <span>k</span>
            </div>
          </div>
        </div>

        <!-- 全屏按钮 -->
        <div class="btn-box screen-box" v-if="shouldShowFullscreen" @click="toggleFullScreen">
          <div
            class="btn"
            :class="{ 'full-screen-btn': !isFullscreen, 'exit-full-screen-btn': isFullscreen }"
          >
            <i class="iconfont-sys">{{ isFullscreen ? '&#xe62d;' : '&#xe8ce;' }}</i>
          </div>
        </div>
        <!-- 通知 -->
        <div class="btn-box notice-btn" v-if="shouldShowNotification" @click="visibleNotice">
          <div class="btn notice-button">
            <i class="iconfont-sys notice-btn">&#xe6c2;</i>
            <span class="count notice-btn"></span>
          </div>
        </div>
        <!-- 聊天 -->
        <div class="btn-box chat-btn" v-if="shouldShowChat" @click="openChat">
          <div class="btn chat-button">
            <i class="iconfont-sys">&#xe89a;</i>
            <span class="dot"></span>
          </div>
        </div>
        <!-- 语言 -->
        <div class="btn-box" v-if="shouldShowLanguage">
          <ElDropdown @command="changeLanguage" popper-class="langDropDownStyle">
            <div class="btn language-btn">
              <i class="iconfont-sys">&#xe611;</i>
            </div>
            <template #dropdown>
              <ElDropdownMenu>
                <div v-for="item in languageOptions" :key="item.value" class="lang-btn-item">
                  <ElDropdownItem
                    :command="item.value"
                    :class="{ 'is-selected': locale === item.value }"
                  >
                    <span class="menu-txt">{{ item.label }}</span>
                    <i v-if="locale === item.value" class="iconfont-sys">&#xe621;</i>
                  </ElDropdownItem>
                </div>
              </ElDropdownMenu>
            </template>
          </ElDropdown>
        </div>
        <!-- 设置 -->
        <div class="btn-box" v-if="shouldShowSettings" @click="openSetting">
          <ElPopover :visible="showSettingGuide" placement="bottom-start" :width="190" :offset="0">
            <template #reference>
              <div class="btn setting-btn">
                <i class="iconfont-sys">&#xe6d0;</i>
              </div>
            </template>
            <template #default>
              <p
                >{{ $t('topBar.guide.title')
                }}<span :style="{ color: systemThemeColor }"> {{ $t('topBar.guide.theme') }} </span
                >、 <span :style="{ color: systemThemeColor }"> {{ $t('topBar.guide.menu') }} </span
                >{{ $t('topBar.guide.description') }}
              </p>
            </template>
          </ElPopover>
        </div>
        <!-- 切换主题 -->
        <div class="btn-box" v-if="shouldShowThemeToggle" @click="themeAnimation">
          <div class="btn theme-btn">
            <i class="iconfont-sys">{{ isDark ? '&#xe6b5;' : '&#xe725;' }}</i>
          </div>
        </div>

        <!-- GitHub仓库链接 -->
        <div class="btn-box" v-if="shouldShowGithubLink" @click="openGithub">
          <div class="btn github-btn">
            <i class="iconfont-sys">&#xe6f6;</i>
          </div>
        </div>

        <!-- PyPI仓库链接 -->
        <div class="btn-box" v-if="shouldShowPypiLink" @click="openPypi">
          <div class="btn pypi-btn">
            <i class="iconfont-sys">&#xe6f7;</i>
          </div>
        </div>

        <!-- 用户系统已移除：隐藏头像与用户菜单 -->
      </div>
    </div>
    <ArtWorkTab />

    <ArtNotification v-model:value="showNotice" ref="notice" />
  </div>
</template>

<script setup lang="ts">
  import { ref, computed, onMounted, onUnmounted } from 'vue'
  import { storeToRefs } from 'pinia'
  import { useI18n } from 'vue-i18n'
  import { useRouter } from 'vue-router'
  import { useFullscreen, useWindowSize } from '@vueuse/core'
  import { LanguageEnum, MenuTypeEnum } from '@/enums/appEnum'
  import { useSettingStore } from '@/store/modules/setting'
  import { useMenuStore } from '@/store/modules/menu'
  import AppConfig from '@/config'
  import { languageOptions } from '@/locales'
  import { WEB_LINKS } from '@/utils/constants'
  import { mittBus } from '@/utils/sys'
  import { themeAnimation } from '@/utils/theme/animation'
  import { useCommon } from '@/composables/useCommon'
  import { useHeaderBar } from '@/composables/useHeaderBar'

  defineOptions({ name: 'ArtHeaderBar' })

  // 检测操作系统类型
  const isWindows = navigator.userAgent.includes('Windows')

  const router = useRouter()
  const { locale, t } = useI18n()
  const { width } = useWindowSize()

  const settingStore = useSettingStore()
  const menuStore = useMenuStore()

  // 顶部栏功能配置
  const {
    shouldShowMenuButton,
    shouldShowRefreshButton,
    shouldShowFastEnter,
    shouldShowBreadcrumb,
    shouldShowGlobalSearch,
    shouldShowFullscreen,
    shouldShowNotification,
    shouldShowChat,
    shouldShowLanguage,
    shouldShowSettings,
    shouldShowThemeToggle,
    shouldShowGithubLink,
    shouldShowPypiLink,
    fastEnterMinWidth: headerBarFastEnterMinWidth
  } = useHeaderBar()

  const { menuOpen, systemThemeColor, showSettingGuide, menuType, isDark, tabStyle } =
    storeToRefs(settingStore)

  const language = locale
  const { menuList } = storeToRefs(menuStore)

  const showNotice = ref(false)
  const notice = ref(null)
  const userMenuPopover = ref()

  // 菜单类型判断
  const isLeftMenu = computed(() => menuType.value === MenuTypeEnum.LEFT)
  const isDualMenu = computed(() => menuType.value === MenuTypeEnum.DUAL_MENU)
  const isTopMenu = computed(() => menuType.value === MenuTypeEnum.TOP)
  const isTopLeftMenu = computed(() => menuType.value === MenuTypeEnum.TOP_LEFT)

  const { isFullscreen, toggle: toggleFullscreen } = useFullscreen()

  onMounted(() => {
    initLanguage()
    document.addEventListener('click', bodyCloseNotice)
  })

  onUnmounted(() => {
    document.removeEventListener('click', bodyCloseNotice)
  })

  /**
   * 切换全屏状态
   */
  const toggleFullScreen = (): void => {
    toggleFullscreen()
  }

  /**
   * 切换菜单显示/隐藏状态
   */
  const visibleMenu = (): void => {
    settingStore.setMenuOpen(!menuOpen.value)
  }

  /**
   * 页面跳转
   * @param {string} path - 目标路径
   */
  const goPage = (path: string): void => {
    router.push(path)
  }

  /**
   * 打开文档页面
   */
  const toDocs = (): void => {
    window.open(WEB_LINKS.DOCS)
  }

  /**
   * 打开 GitHub 页面
   */
  const toGithub = (): void => {
    window.open(WEB_LINKS.GITHUB)
  }

  /**
   * 打开 GitHub 仓库页面
   */
  const openGithub = (): void => {
    window.open('https://github.com/whillhill/mcpstore', '_blank')
  }

  /**
   * 打开 PyPI 仓库页面
   */
  const openPypi = (): void => {
    window.open('https://pypi.org/project/mcpstore', '_blank')
  }

  /**
   * 跳转到首页
   */
  const toHome = (): void => {
    router.push(useCommon().homePath.value)
  }

  /**
   * 用户登出确认
   */
  const loginOut = (): void => {
    // [MCP-only] 无登录/鉴权流程：登出逻辑省略
    closeUserMenu()
  }

  /**
   * 刷新页面
   * @param {number} time - 延迟时间，默认为0毫秒
   */
  const reload = (time: number = 0): void => {
    setTimeout(() => {
      useCommon().refresh()
    }, time)
  }

  /**
   * 初始化语言设置
   */
  const initLanguage = (): void => {
    // [MCP-only] 语言由 i18n 管理，已在全局初始化
  }

  /**
   * 切换系统语言
   * @param {LanguageEnum} lang - 目标语言类型
   */
  const changeLanguage = (lang: LanguageEnum): void => {
    if (locale.value === lang) return
    locale.value = lang
    // [MCP-only] 不再持久化到用户 Store；根据需要可加入本地存储
    reload(50)
  }

  /**
   * 打开设置面板
   */
  const openSetting = (): void => {
    mittBus.emit('openSetting')

    // 隐藏设置引导提示
    if (showSettingGuide.value) {
      settingStore.hideSettingGuide()
    }
  }

  /**
   * 打开全局搜索对话框
   */
  const openSearchDialog = (): void => {
    mittBus.emit('openSearchDialog')
  }

  /**
   * 点击页面其他区域关闭通知面板
   * @param {Event} e - 点击事件对象
   */
  const bodyCloseNotice = (e: any): void => {
    let { className } = e.target

    if (showNotice.value) {
      if (typeof className === 'object') {
        showNotice.value = false
        return
      }
      if (className.indexOf('notice-btn') === -1) {
        showNotice.value = false
      }
    }
  }

  /**
   * 切换通知面板显示状态
   */
  const visibleNotice = (): void => {
    showNotice.value = !showNotice.value
  }

  /**
   * 打开聊天窗口
   */
  const openChat = (): void => {
    mittBus.emit('openChat')
  }

  /**
   * 打开锁屏功能
   */
  const lockScreen = (): void => {
    mittBus.emit('openLockScreen')
  }

  /**
   * 关闭用户菜单弹出层
   */
  const closeUserMenu = (): void => {
    setTimeout(() => {
      userMenuPopover.value.hide()
    }, 100)
  }
</script>

<style lang="scss" scoped>
  @use './style';
  @use './mobile';
</style>
