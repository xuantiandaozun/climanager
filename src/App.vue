<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { computed, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

const { t, locale } = useI18n()

type Tool = {
  id: number
  name: string
  command: string
  detected_path: string | null
  enabled: boolean
}

type Workspace = {
  id: number
  name: string
  path: string
  created_at: string
  last_opened_at: string | null
  favorite: boolean
  group_name: string
  archived: boolean
  default_tool_id: number | null
}

type WorkspaceView = 'all' | 'recent' | 'favorites' | 'archived'

const workspaceViews: WorkspaceView[] = ['all', 'recent', 'favorites', 'archived']

type LaunchRecord = {
  id: number
  workspace_name: string
  workspace_path: string
  tool_name: string
  command: string
  launched_at: string
  proxy_enabled: boolean
  proxy_url: string | null
}

type SessionRecord = {
  id: number
  workspace_name: string
  tool_name: string
  title: string
  source_path: string
  updated_at: string
  matched_by: string
}

type ToolSessionList = {
  command: string
  output: string
  stderr: string
  lines: string[]
  sessions: ToolSessionItem[]
}

type ToolSessionItem = {
  id: string
  title: string
  updated: string
  line: string
}

type ProxyConfig = {
  enabled: boolean
  host: string
  port: string
}

type PageId = 'overview' | 'workspaces' | 'tools' | 'proxy' | 'records'

const workspaces = ref<Workspace[]>([])
const tools = ref<Tool[]>([])
const history = ref<LaunchRecord[]>([])
const sessions = ref<SessionRecord[]>([])
const proxy = ref<ProxyConfig>({ enabled: false, host: '127.0.0.1', port: '7890' })
const selectedWorkspaceId = ref<number | null>(null)
const selectedToolId = ref<number | null>(null)
const workspaceSearch = ref('')
const workspaceView = ref<WorkspaceView>('all')
const workspaceGroupFilter = ref('all')
const editingWorkspace = ref<Workspace | null>(null)
const workspaceNameDraft = ref('')
const workspaceGroupDraft = ref('')
const sessionModalWorkspace = ref<Workspace | null>(null)
const modalToolSessions = ref<ToolSessionList | null>(null)
const modalSelectedToolId = ref<number | null>(null)
const isLoadingModalSessions = ref(false)
const statusMessage = ref('')
const isBusy = ref(false)
const currentPage = ref<PageId>('overview')
const overviewRecentWorkspaceIds = ref<number[]>([])

const navPages: { id: PageId }[] = [
  { id: 'overview' },
  { id: 'workspaces' },
  { id: 'tools' },
  { id: 'proxy' },
  { id: 'records' },
]

const selectedWorkspace = computed(() =>
  workspaces.value.find((workspace) => workspace.id === selectedWorkspaceId.value),
)

const selectedTool = computed(() => tools.value.find((tool) => tool.id === selectedToolId.value))

const readyToolCount = computed(() => tools.value.filter((tool) => tool.detected_path).length)

const proxyPreview = computed(() => {
  if (!proxy.value.enabled) return t('time.not_enabled')
  if (!proxy.value.host || !proxy.value.port) return t('time.incomplete')
  return `http://${proxy.value.host}:${proxy.value.port}`
})

const normalizedWorkspaceSearch = computed(() => workspaceSearch.value.trim().toLowerCase())

const activeWorkspaces = computed(() => workspaces.value.filter((workspace) => !workspace.archived))

const archivedWorkspaces = computed(() => workspaces.value.filter((workspace) => workspace.archived))

const recentWorkspaces = computed(() =>
  [...activeWorkspaces.value]
    .sort((left, right) => workspaceSortValue(right) - workspaceSortValue(left))
    .slice(0, 6),
)

const overviewRecentWorkspaces = computed(() =>
  overviewRecentWorkspaceIds.value
    .map((id) => workspaces.value.find((workspace) => workspace.id === id))
    .filter((workspace): workspace is Workspace => Boolean(workspace)),
)

const workspaceViewCounts = computed(() => ({
  all: activeWorkspaces.value.length,
  recent: Math.min(activeWorkspaces.value.length, 12),
  favorites: activeWorkspaces.value.filter((workspace) => workspace.favorite).length,
  archived: archivedWorkspaces.value.length,
}))

const workspaceBaseList = computed(() => {
  if (workspaceView.value === 'archived') return archivedWorkspaces.value
  if (workspaceView.value === 'favorites') return activeWorkspaces.value.filter((workspace) => workspace.favorite)
  if (workspaceView.value === 'recent') {
    return [...activeWorkspaces.value]
      .sort((left, right) => workspaceSortValue(right) - workspaceSortValue(left))
      .slice(0, 12)
  }

  return activeWorkspaces.value
})

const workspaceGroupOptions = computed(() => {
  const groups = new Set<string>()
  for (const workspace of workspaceBaseList.value) {
    groups.add(workspace.group_name.trim())
  }

  return Array.from(groups)
    .sort((left, right) => left.localeCompare(right))
    .map((group) => ({ value: group, label: workspaceGroupLabel(group) }))
})

const visibleWorkspaces = computed(() => {
  let items = [...workspaceBaseList.value]

  if (workspaceGroupFilter.value !== 'all') {
    items = items.filter((workspace) => workspace.group_name.trim() === workspaceGroupFilter.value)
  }

  if (normalizedWorkspaceSearch.value) {
    items = items.filter((workspace) => {
      const haystack = `${workspace.name} ${workspace.path}`.toLowerCase()
      return haystack.includes(normalizedWorkspaceSearch.value)
    })
  }

  return items
})

const groupedVisibleWorkspaces = computed(() => {
  const groups = new Map<string, Workspace[]>()

  for (const workspace of visibleWorkspaces.value) {
    const key = workspace.group_name.trim()
    const current = groups.get(key) ?? []
    current.push(workspace)
    groups.set(key, current)
  }

  return Array.from(groups.entries())
    .sort((a, b) => a[0].localeCompare(b[0]))
    .map(([groupName, items]) => ({
      key: groupName || '__ungrouped__',
      label: workspaceGroupLabel(groupName),
      items,
    }))
})

const groupTabList = computed(() => {
  const groups = new Set<string>()
  for (const workspace of workspaceBaseList.value) {
    groups.add(workspace.group_name.trim())
  }
  const sorted = Array.from(groups).sort((a, b) => a.localeCompare(b))
  return [
    { value: 'all', label: t('workspaces.all_groups'), count: workspaceBaseList.value.length },
    ...sorted.map((g) => ({
      value: g,
      label: workspaceGroupLabel(g),
      count: workspaceBaseList.value.filter((w) => w.group_name.trim() === g).length,
    })),
  ]
})

onMounted(() => {
  statusMessage.value = t('status.ready')
  void refreshAll()
})

watch(editingWorkspace, (workspace) => {
  workspaceNameDraft.value = workspace?.name ?? ''
  workspaceGroupDraft.value = workspace?.group_name ?? ''
}, { immediate: true })

watch(workspaceGroupOptions, (options) => {
  if (workspaceGroupFilter.value === 'all') return
  if (!options.some((option) => option.value === workspaceGroupFilter.value)) {
    workspaceGroupFilter.value = 'all'
  }
}, { immediate: true })

watch(currentPage, async (page) => {
  if (page === 'overview' || page === 'workspaces') {
    try {
      await refreshWorkspaces({ syncOverview: true })
    } catch (error) {
      statusMessage.value = readableError(error)
    }
  }
})

function switchLang(lang: string) {
  locale.value = lang
  document.documentElement.lang = lang
  statusMessage.value = t('status.ready')
}

async function refreshAll() {
  isBusy.value = true
  try {
    const [workspaceResult, toolResult, proxyResult, historyResult, sessionResult] = await Promise.all([
      invoke<Workspace[]>('list_workspaces'),
      invoke<Tool[]>('list_tools'),
      invoke<ProxyConfig>('get_proxy_config'),
      invoke<LaunchRecord[]>('list_launch_history'),
      invoke<SessionRecord[]>('list_sessions'),
    ])

    workspaces.value = workspaceResult
    tools.value = toolResult
    proxy.value = proxyResult
    history.value = historyResult
    sessions.value = sessionResult
    syncOverviewRecentWorkspaces()
    syncSelectedWorkspace()
    syncSelectedTool()
    statusMessage.value = t('status.synced')
  } catch (error) {
    statusMessage.value = readableError(error)
  } finally {
    isBusy.value = false
  }
}

async function refreshWorkspaces(options: { syncOverview?: boolean } = {}) {
  workspaces.value = await invoke<Workspace[]>('list_workspaces')
  syncSelectedWorkspace()
  if (options.syncOverview) {
    syncOverviewRecentWorkspaces()
  }
}

async function scanSessions() {
  isBusy.value = true
  try {
    sessions.value = await invoke<SessionRecord[]>('scan_sessions')
    statusMessage.value = sessions.value.length
      ? t('status.indexed_sessions', { count: sessions.value.length })
      : t('status.no_sessions_found')
  } catch (error) {
    statusMessage.value = readableError(error)
  } finally {
    isBusy.value = false
  }
}

async function addWorkspace() {
  const selected = await open({ directory: true, multiple: false, title: 'Select workspace' })
  if (typeof selected !== 'string') return

  isBusy.value = true
  try {
    workspaces.value = await invoke<Workspace[]>('add_workspace', {
      input: { path: selected },
    })
    selectedWorkspaceId.value =
      workspaces.value.find((workspace) => workspace.path.toLowerCase() === selected.replaceAll('\\', '/').toLowerCase())?.id ??
      workspaces.value[0]?.id ??
      null
    statusMessage.value = t('status.workspace_saved')
  } catch (error) {
    statusMessage.value = readableError(error)
  } finally {
    isBusy.value = false
  }
}

async function removeWorkspace(workspace: Workspace) {
  isBusy.value = true
  try {
    workspaces.value = await invoke<Workspace[]>('delete_workspace', { id: workspace.id })
    if (selectedWorkspaceId.value === workspace.id) {
      selectedWorkspaceId.value = workspaces.value[0]?.id ?? null
    }
    statusMessage.value = t('status.workspace_removed', { name: workspace.name })
  } catch (error) {
    statusMessage.value = readableError(error)
  } finally {
    isBusy.value = false
  }
}

async function toggleWorkspaceFavorite(workspace: Workspace) {
  isBusy.value = true
  try {
    workspaces.value = await invoke<Workspace[]>('toggle_workspace_favorite', { id: workspace.id })
    statusMessage.value = workspace.favorite
      ? t('status.workspace_unfavorited', { name: workspace.name })
      : t('status.workspace_favorited', { name: workspace.name })
  } catch (error) {
    statusMessage.value = readableError(error)
  } finally {
    isBusy.value = false
  }
}

async function toggleWorkspaceArchived(workspace: Workspace) {
  isBusy.value = true
  try {
    workspaces.value = await invoke<Workspace[]>('toggle_workspace_archived', { id: workspace.id })
    statusMessage.value = workspace.archived
      ? t('status.workspace_unarchived', { name: workspace.name })
      : t('status.workspace_archived', { name: workspace.name })
  } catch (error) {
    statusMessage.value = readableError(error)
  } finally {
    isBusy.value = false
  }
}

function openEditModal(workspace: Workspace) {
  editingWorkspace.value = workspace
}

function closeEditModal() {
  editingWorkspace.value = null
}

async function saveWorkspaceDetails() {
  if (!editingWorkspace.value) return

  isBusy.value = true
  try {
    workspaces.value = await invoke<Workspace[]>('update_workspace', {
      input: {
        id: editingWorkspace.value.id,
        name: workspaceNameDraft.value,
        group_name: workspaceGroupDraft.value,
      },
    })
    statusMessage.value = t('status.workspace_updated', { name: workspaceNameDraft.value.trim() || editingWorkspace.value.name })
    closeEditModal()
  } catch (error) {
    statusMessage.value = readableError(error)
  } finally {
    isBusy.value = false
  }
}

async function openInExplorer(workspace: Workspace) {
  isBusy.value = true
  try {
    await invoke('open_in_explorer', { id: workspace.id })
    statusMessage.value = t('status.opened_in_explorer', { name: workspace.name })
  } catch (error) {
    statusMessage.value = readableError(error)
  } finally {
    isBusy.value = false
  }
}

async function openWorkspaceInVSCode(workspace: Workspace) {
  isBusy.value = true
  try {
    workspaces.value = await invoke<Workspace[]>('open_workspace_in_vscode', { id: workspace.id })
    selectedWorkspaceId.value = workspace.id
    statusMessage.value = t('status.opened_in_vscode', { name: workspace.name })
  } catch (error) {
    statusMessage.value = readableError(error)
  } finally {
    isBusy.value = false
  }
}

async function saveProxy() {
  isBusy.value = true
  try {
    proxy.value = await invoke<ProxyConfig>('save_proxy_config', { config: proxy.value })
    statusMessage.value = proxy.value.enabled
      ? t('status.proxy_saved', { url: proxyPreview.value })
      : t('status.proxy_disabled')
  } catch (error) {
    statusMessage.value = readableError(error)
  } finally {
    isBusy.value = false
  }
}

async function launchSelected() {
  if (!selectedWorkspace.value || !selectedTool.value) {
    statusMessage.value = t('status.select_workspace_and_tool')
    return
  }

  isBusy.value = true
  try {
    history.value = await invoke<LaunchRecord[]>('launch_tool', {
      input: {
        workspace_id: selectedWorkspace.value.id,
        tool_id: selectedTool.value.id,
      },
    })
    statusMessage.value = t('status.launched_in', {
      name: selectedWorkspace.value.name,
      tool: selectedTool.value.name,
    })
  } catch (error) {
    statusMessage.value = readableError(error)
  } finally {
    isBusy.value = false
  }
}

async function launchWorkspace(workspace: Workspace) {
  const toolId = workspace.default_tool_id ?? selectedToolId.value
  if (!toolId) {
    statusMessage.value = t('status.select_workspace_and_tool')
    return
  }

  isBusy.value = true
  try {
    history.value = await invoke<LaunchRecord[]>('launch_tool', {
      input: {
        workspace_id: workspace.id,
        tool_id: toolId,
      },
    })
    const tool = tools.value.find((t) => t.id === toolId)
    statusMessage.value = t('status.launched_in', {
      name: workspace.name,
      tool: tool?.name ?? '',
    })
  } catch (error) {
    statusMessage.value = readableError(error)
  } finally {
    isBusy.value = false
  }
}

async function setWorkspaceDefaultTool(workspace: Workspace, toolId: number | null) {
  isBusy.value = true
  try {
    workspaces.value = await invoke<Workspace[]>('set_workspace_default_tool', {
      workspaceId: workspace.id,
      toolId,
    })
    if (toolId === null) {
      statusMessage.value = t('status.workspace_tool_cleared', { name: workspace.name })
    } else {
      const tool = tools.value.find((t) => t.id === toolId)
      statusMessage.value = t('status.workspace_tool_set', { name: workspace.name, tool: tool?.name ?? toolId })
    }
  } catch (error) {
    statusMessage.value = readableError(error)
  } finally {
    isBusy.value = false
  }
}

function openSessionModal(workspace: Workspace) {
  sessionModalWorkspace.value = workspace
  modalSelectedToolId.value = workspace.default_tool_id ?? selectedToolId.value
  modalToolSessions.value = null
  loadModalToolSessions()
}

function closeSessionModal() {
  sessionModalWorkspace.value = null
  modalToolSessions.value = null
  modalSelectedToolId.value = null
}

async function loadModalToolSessions() {
  const workspace = sessionModalWorkspace.value
  const toolId = modalSelectedToolId.value
  if (!workspace || !toolId) {
    statusMessage.value = t('status.select_workspace_and_tool')
    return
  }

  isLoadingModalSessions.value = true
  try {
    modalToolSessions.value = await invoke<ToolSessionList>('list_tool_sessions', {
      input: {
        workspace_id: workspace.id,
        tool_id: toolId,
      },
    })
    statusMessage.value = modalToolSessions.value.lines.length
      ? t('status.read_session_records', { count: modalToolSessions.value.lines.length })
      : t('status.no_session_records')
  } catch (error) {
    statusMessage.value = readableError(error)
  } finally {
    isLoadingModalSessions.value = false
  }
}

async function openModalToolSession(session: ToolSessionItem) {
  const workspace = sessionModalWorkspace.value
  const toolId = modalSelectedToolId.value
  if (!workspace || !toolId) {
    statusMessage.value = t('status.select_workspace_and_tool')
    return
  }

  isBusy.value = true
  try {
    await invoke('open_tool_session', {
      input: {
        workspace_id: workspace.id,
        tool_id: toolId,
        session_id: session.id,
      },
    })
    statusMessage.value = t('status.session_opened', { title: session.title })
  } catch (error) {
    statusMessage.value = readableError(error)
  } finally {
    isBusy.value = false
  }
}

function formatTime(value: string | null) {
  if (!value) return t('time.not_launched')
  return new Intl.DateTimeFormat(locale.value, {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(value))
}

function workspaceSortValue(workspace: Workspace) {
  return Date.parse(workspace.last_opened_at ?? workspace.created_at) || 0
}

function syncOverviewRecentWorkspaces() {
  overviewRecentWorkspaceIds.value = recentWorkspaces.value.map((workspace) => workspace.id)
}

function syncSelectedWorkspace() {
  if (!workspaces.value.some((workspace) => workspace.id === selectedWorkspaceId.value)) {
    selectedWorkspaceId.value = workspaces.value[0]?.id ?? null
  }
}

function syncSelectedTool() {
  if (!tools.value.some((tool) => tool.id === selectedToolId.value)) {
    selectedToolId.value = tools.value[0]?.id ?? null
  }
}

function workspaceFavoriteLabel(workspace: Workspace) {
  return workspace.favorite ? t('button.unfavorite') : t('button.favorite')
}

function workspaceArchiveLabel(workspace: Workspace) {
  return workspace.archived ? t('button.unarchive') : t('button.archive')
}

function workspaceGroupLabel(groupName: string) {
  return groupName.trim() || t('workspaces.ungrouped_group')
}

function readableError(error: unknown) {
  return error instanceof Error ? error.message : String(error)
}
</script>

<template>
  <main class="shell">
    <aside class="rail" aria-label="CLI Manager navigation">
      <div class="brand">
        <img src="/logo.png" alt="CLI Manager" class="brand-logo" />
        <div>
          <p>CLI Manager</p>
          <small>{{ $t('sidebar.subtitle') }}</small>
        </div>
      </div>

      <nav class="nav-list">
        <button
          v-for="page in navPages"
          :key="page.id"
          :class="['nav-item', { active: currentPage === page.id }]"
          type="button"
          @click="currentPage = page.id"
        >
          <span>{{ $t('nav.' + page.id) }}</span>
          <small>{{ $t('nav.' + page.id + '_desc') }}</small>
        </button>
      </nav>

      <div class="lang-switcher">
        <button
          :class="['lang-btn', { active: locale === 'zh-CN' }]"
          @click="switchLang('zh-CN')"
        >中文</button>
        <span class="lang-divider">/</span>
        <button
          :class="['lang-btn', { active: locale === 'en' }]"
          @click="switchLang('en')"
        >EN</button>
      </div>

      <div class="open-source-card">
        <span>{{ $t('sidebar.status_label') }}</span>
        <strong>{{ statusMessage }}</strong>
      </div>
    </aside>

    <section class="content">
      <header class="page-header">
        <div>
          <p class="eyebrow">{{ $t('page_header.subtitle') }}</p>
          <h1>{{ $t('page_header.' + currentPage) }}</h1>
        </div>
        <div class="hero-actions">
          <button class="ghost" :disabled="isBusy" @click="refreshAll">{{ $t('button.refresh') }}</button>
        </div>
      </header>

      <section v-if="currentPage === 'overview'" class="page-stack">
        <div class="summary-grid">
          <article class="summary-card">
            <span>{{ $t('overview.workspaces') }}</span>
            <strong>{{ workspaces.length }}</strong>
            <button class="text-button" type="button" @click="currentPage = 'workspaces'">{{ $t('button.manage_workspaces') }}</button>
          </article>
          <article class="summary-card">
            <span>{{ $t('overview.available_tools') }}</span>
            <strong>{{ readyToolCount }} / {{ tools.length }}</strong>
            <button class="text-button" type="button" @click="currentPage = 'tools'">{{ $t('button.view_tools') }}</button>
          </article>
          <article class="summary-card">
            <span>{{ $t('overview.proxy') }}</span>
            <strong>{{ proxyPreview }}</strong>
            <button class="text-button" type="button" @click="currentPage = 'proxy'">{{ $t('button.configure_proxy') }}</button>
          </article>
          <article class="summary-card">
            <span>{{ $t('overview.session_index') }}</span>
            <strong>{{ sessions.length }}</strong>
            <button class="text-button" type="button" @click="currentPage = 'records'">{{ $t('button.view_records') }}</button>
          </article>
        </div>

        <section class="panel launch-panel">
          <div class="panel-heading compact">
            <div>
              <p class="eyebrow">{{ $t('launch.quick_launch') }}</p>
              <h2>{{ $t('launch.quick_launch') }}</h2>
            </div>
          </div>

          <div class="launch-workspace-section">
            <div class="choice-heading">
              <span>{{ $t('launch.current_workspace') }}</span>
              <strong>{{ selectedWorkspace?.name ?? $t('launch.not_selected') }}</strong>
            </div>
            <div v-if="overviewRecentWorkspaces.length" class="workspace-grid">
              <article
                v-for="workspace in overviewRecentWorkspaces"
                :key="workspace.id"
                :class="['workspace-card', { selected: workspace.id === selectedWorkspaceId }]"
                @click="selectedWorkspaceId = workspace.id; if (workspace.default_tool_id) selectedToolId = workspace.default_tool_id"
              >
                <div class="workspace-topline">
                  <div class="workspace-topline-left">
                    <span class="folder-dot"></span>
                    <span v-if="workspace.favorite" class="workspace-badge">{{ $t('workspaces.favorite_badge') }}</span>
                  </div>
                  <span>{{ formatTime(workspace.last_opened_at) }}</span>
                </div>
                <h3>{{ workspace.name }}</h3>
                <p>{{ workspace.path }}</p>
                <div class="workspace-default-tool" @click.stop>
                  <label class="workspace-tool-label">{{ $t('workspaces.default_tool_label') }}</label>
                  <select
                    class="workspace-tool-select"
                    :value="workspace.default_tool_id ?? ''"
                    :disabled="isBusy"
                    @change="setWorkspaceDefaultTool(workspace, ($event.target as HTMLSelectElement).value ? Number(($event.target as HTMLSelectElement).value) : null)"
                  >
                    <option value="">{{ $t('workspaces.default_tool_none') }}</option>
                    <option v-for="tool in tools" :key="tool.id" :value="tool.id">
                      {{ tool.name }}
                    </option>
                  </select>
                </div>
                <div class="workspace-meta">
                  <div class="workspace-actions">
                    <button class="mini-button primary" :disabled="isBusy" @click.stop="launchWorkspace(workspace)">
                      {{ $t('button.launch') }}
                    </button>
                    <button class="mini-button" :disabled="isBusy" @click.stop="openSessionModal(workspace)">
                      {{ $t('button.view_sessions') }}
                    </button>
                    <button class="mini-button" :disabled="isBusy" @click.stop="openWorkspaceInVSCode(workspace)">
                      {{ $t('button.open_in_vscode_short') }}
                    </button>
                    <button class="mini-button" :disabled="isBusy" @click.stop="openInExplorer(workspace)">
                      {{ $t('button.open_in_explorer') }}
                    </button>
                  </div>
                </div>
              </article>
            </div>
            <p v-else>{{ $t('launch.no_workspaces_hint') }}</p>
          </div>


        </section>
      </section>

      <section v-else-if="currentPage === 'workspaces'" class="page-stack">
        <section class="panel workspace-panel">
          <div class="panel-heading">
            <div>
              <p class="eyebrow">{{ $t('launch.quick_launch') }}</p>
              <h2>{{ $t('workspaces.title') }}</h2>
            </div>
            <div class="panel-actions">
              <button class="ghost" :disabled="isBusy" @click="addWorkspace">{{ $t('button.add_workspace') }}</button>
              <button
                class="ghost"
                :disabled="isBusy || !selectedWorkspace"
                @click="selectedWorkspace && openWorkspaceInVSCode(selectedWorkspace)"
              >
                {{ $t('button.open_in_vscode') }}
              </button>
              <button class="ghost" :disabled="!selectedWorkspace" @click="selectedWorkspace && openSessionModal(selectedWorkspace)">
                {{ $t('button.view_sessions') }}
              </button>
              <select
                v-if="tools.length"
                class="tool-inline-select"
                :value="selectedToolId ?? ''"
                @change="selectedToolId = Number(($event.target as HTMLSelectElement).value) || null"
              >
                <option v-for="tool in tools" :key="tool.id" :value="tool.id">
                  {{ tool.name }}{{ tool.detected_path ? '' : ' ⚠' }}
                </option>
              </select>
              <button class="primary" :disabled="isBusy" @click="launchSelected">{{ $t('button.launch_selected') }}</button>
            </div>
          </div>

          <div class="workspace-toolbar">
            <div class="view-switcher" role="tablist" aria-label="Workspace views">
              <button
                v-for="view in workspaceViews"
                :key="view"
                :class="['view-chip', { selected: workspaceView === view }]"
                type="button"
                @click="workspaceView = view"
              >
                <span>{{ $t(`workspaces.${view}_tab`) }}</span>
                <small>{{ workspaceViewCounts[view] }}</small>
              </button>
            </div>

            <label class="workspace-search">
              <span>{{ $t('workspaces.search_label') }}</span>
              <input v-model="workspaceSearch" type="text" :placeholder="$t('workspaces.search_placeholder')" />
            </label>

          </div>

          <div class="group-tabs" role="tablist" aria-label="Workspace groups">
            <button
              v-for="tab in groupTabList"
              :key="tab.value || '__ungrouped__'"
              :class="['group-tab', { selected: workspaceGroupFilter === tab.value }]"
              type="button"
              @click="workspaceGroupFilter = tab.value"
            >
              <span>{{ tab.label }}</span>
              <small>{{ tab.count }}</small>
            </button>
          </div>

          <p class="workspace-results" v-if="workspaces.length">
            {{ $t('workspaces.results_count', { count: visibleWorkspaces.length }) }}
          </p>

          <div v-if="visibleWorkspaces.length" class="workspace-grid">
                <article
                  v-for="workspace in visibleWorkspaces"
                  :key="workspace.id"
                  :class="['workspace-card', { selected: workspace.id === selectedWorkspaceId, archived: workspace.archived }]"
                  @click="selectedWorkspaceId = workspace.id; if (workspace.default_tool_id) selectedToolId = workspace.default_tool_id"
                >
                  <div class="workspace-topline">
                    <div class="workspace-topline-left">
                      <span class="folder-dot"></span>
                      <span v-if="workspace.favorite" class="workspace-badge">{{ $t('workspaces.favorite_badge') }}</span>
                      <span v-if="workspace.archived" class="workspace-badge muted">{{ $t('workspaces.archived_badge') }}</span>
                    </div>
                    <span>{{ formatTime(workspace.last_opened_at) }}</span>
                  </div>
                  <h3>{{ workspace.name }}</h3>
                  <p>{{ workspace.path }}</p>
                  <div class="workspace-default-tool" @click.stop>
                    <label class="workspace-tool-label">{{ $t('workspaces.default_tool_label') }}</label>
                    <select
                      class="workspace-tool-select"
                      :value="workspace.default_tool_id ?? ''"
                      :disabled="isBusy"
                      @change="setWorkspaceDefaultTool(workspace, ($event.target as HTMLSelectElement).value ? Number(($event.target as HTMLSelectElement).value) : null)"
                    >
                      <option value="">{{ $t('workspaces.default_tool_none') }}</option>
                      <option v-for="tool in tools" :key="tool.id" :value="tool.id">
                        {{ tool.name }}
                      </option>
                    </select>
                  </div>
                  <div class="workspace-meta">
                    <div class="workspace-actions">
                      <button class="mini-button primary" :disabled="isBusy" @click.stop="launchWorkspace(workspace)">
                        {{ $t('button.launch') }}
                      </button>
                      <button class="mini-button" :disabled="isBusy" @click.stop="openSessionModal(workspace)">
                        {{ $t('button.view_sessions') }}
                      </button>
                      <button class="mini-button" :disabled="isBusy" @click.stop="openEditModal(workspace)">
                        {{ $t('button.edit') }}
                      </button>
                      <button class="mini-button" :disabled="isBusy" @click.stop="toggleWorkspaceFavorite(workspace)">
                        {{ workspaceFavoriteLabel(workspace) }}
                      </button>
                      <button class="mini-button" :disabled="isBusy" @click.stop="toggleWorkspaceArchived(workspace)">
                        {{ workspaceArchiveLabel(workspace) }}
                      </button>
                      <button class="mini-button" :disabled="isBusy" @click.stop="openWorkspaceInVSCode(workspace)">
                        {{ $t('button.open_in_vscode_short') }}
                      </button>
                      <button class="mini-button" :disabled="isBusy" @click.stop="openInExplorer(workspace)">
                        {{ $t('button.open_in_explorer') }}
                      </button>
                      <button class="mini-button" :disabled="isBusy" @click.stop="removeWorkspace(workspace)">
                        {{ $t('button.remove') }}
                      </button>
                    </div>
                  </div>
                </article>
          </div>

          <div v-else class="empty-state">
            <strong>{{ workspaces.length ? $t('workspaces.no_filtered_title') : $t('workspaces.no_workspaces_title') }}</strong>
            <p>{{ workspaces.length ? $t('workspaces.no_filtered_hint') : $t('workspaces.no_workspaces_hint') }}</p>
          </div>
        </section>
      </section>

      <section v-else-if="currentPage === 'tools'" class="page-stack">
        <section class="panel">
          <div class="panel-heading compact">
            <div>
              <p class="eyebrow">{{ $t('tools.title') }}</p>
              <h2>{{ $t('tools.title') }}</h2>
            </div>
          </div>
          <div class="tool-list">
            <article
              v-for="tool in tools"
              :key="tool.id"
              :class="['tool-row', { selected: tool.id === selectedToolId }]"
              @click="selectedToolId = tool.id"
            >
              <div>
                <strong>{{ tool.name }}</strong>
                <code>{{ tool.command }}</code>
                <p>{{ tool.detected_path ?? $t('tools.not_detected') }}</p>
              </div>
              <span :class="['status', tool.detected_path ? 'ready' : 'missing']">
                {{ tool.detected_path ? $t('tools.ready') : $t('tools.missing') }}
              </span>
            </article>
          </div>
        </section>
      </section>

      <section v-else-if="currentPage === 'proxy'" class="page-stack narrow-page">
        <section class="panel proxy-panel">
          <div class="panel-heading compact">
            <div>
              <p class="eyebrow">{{ $t('proxy.title') }}</p>
              <h2>{{ $t('proxy.title') }}</h2>
            </div>
          </div>

          <form class="proxy-form" @submit.prevent="saveProxy">
            <label class="switch-row">
              <input v-model="proxy.enabled" type="checkbox" />
              <span>{{ $t('proxy.auto_set') }}</span>
            </label>
            <label>
              <span>{{ $t('proxy.host') }}</span>
              <input v-model="proxy.host" placeholder="127.0.0.1" />
            </label>
            <label>
              <span>{{ $t('proxy.port') }}</span>
              <input v-model="proxy.port" placeholder="7890" />
            </label>
            <p>{{ $t('proxy.current', { preview: proxyPreview }) }}</p>
            <button class="primary" :disabled="isBusy">{{ $t('button.save_proxy') }}</button>
          </form>
        </section>
      </section>

      <section v-else class="page-stack">
        <section class="panel history-panel">
          <div class="panel-heading compact">
            <div>
              <p class="eyebrow">{{ $t('records.launch_history') }}</p>
              <h2>{{ $t('records.launch_history') }}</h2>
            </div>
          </div>
          <ol v-if="history.length" class="timeline">
            <li v-for="record in history" :key="record.id">
              <span>{{ formatTime(record.launched_at) }}</span>
              <div>
                <strong>{{ record.workspace_name }} / {{ record.tool_name }}</strong>
                <p>
                  {{ record.command }} · {{ record.workspace_path }}
                  <template v-if="record.proxy_enabled"> · proxy {{ record.proxy_url }}</template>
                </p>
              </div>
            </li>
          </ol>
          <div v-else class="empty-state compact-empty">
            <strong>{{ $t('records.no_history_title') }}</strong>
            <p>{{ $t('records.no_history_hint') }}</p>
          </div>
        </section>

        <section class="panel history-panel">
          <div class="panel-heading compact">
            <div>
              <p class="eyebrow">{{ $t('records.session_index') }}</p>
              <h2>{{ $t('records.session_index') }}</h2>
            </div>
            <button class="text-button" :disabled="isBusy" @click="scanSessions">{{ $t('button.rescan') }}</button>
          </div>
          <ol v-if="sessions.length" class="timeline session-list">
            <li v-for="session in sessions" :key="session.id">
              <span>{{ formatTime(session.updated_at) }}</span>
              <div>
                <strong>{{ session.workspace_name }} / {{ session.tool_name }} / {{ session.title }}</strong>
                <p>{{ session.source_path }} · {{ session.matched_by }}</p>
              </div>
            </li>
          </ol>
          <div v-else class="empty-state compact-empty">
            <strong>{{ $t('records.no_sessions_title') }}</strong>
            <p>{{ $t('records.no_sessions_hint') }}</p>
          </div>
        </section>
      </section>
    </section>

    <div v-if="editingWorkspace" class="modal-overlay" @click.self="closeEditModal">
      <div class="modal-dialog">
        <div class="modal-header">
          <div>
            <p class="eyebrow">{{ $t('workspaces.editor_title') }}</p>
            <h2>{{ editingWorkspace.name }}</h2>
            <p class="panel-note">{{ editingWorkspace.path }}</p>
          </div>
          <button class="modal-close" type="button" @click="closeEditModal">&times;</button>
        </div>

        <form class="workspace-form" @submit.prevent="saveWorkspaceDetails">
          <label>
            <span>{{ $t('workspaces.name_label') }}</span>
            <input v-model="workspaceNameDraft" type="text" :placeholder="$t('workspaces.name_placeholder')" />
          </label>
          <label>
            <span>{{ $t('workspaces.group_label') }}</span>
            <input v-model="workspaceGroupDraft" type="text" :placeholder="$t('workspaces.group_placeholder')" />
          </label>
          <div class="workspace-form-meta">
            <span>{{ editingWorkspace.archived ? $t('workspaces.archived_badge') : $t('workspaces.active_badge') }}</span>
            <span>{{ workspaceGroupLabel(editingWorkspace.group_name) }}</span>
          </div>
          <div class="panel-actions">
            <button class="primary" :disabled="isBusy">{{ $t('button.save_workspace') }}</button>
            <button class="ghost" type="button" :disabled="isBusy" @click="closeEditModal">
              {{ $t('button.cancel') }}
            </button>
          </div>
        </form>
      </div>
    </div>

    <div v-if="sessionModalWorkspace" class="modal-overlay" @click.self="closeSessionModal">
      <div class="modal-dialog modal-dialog-wide">
        <div class="modal-header">
          <div>
            <p class="eyebrow">{{ $t('workspaces.session_list') }}</p>
            <h2>{{ sessionModalWorkspace.name }}</h2>
            <p class="panel-note">{{ sessionModalWorkspace.path }}</p>
          </div>
          <button class="modal-close" type="button" @click="closeSessionModal">&times;</button>
        </div>

        <div class="session-modal-toolbar">
          <div class="session-modal-tool-select">
            <label>{{ $t('workspaces.tool_selector') }}</label>
            <select
              :value="modalSelectedToolId ?? ''"
              :disabled="isLoadingModalSessions"
              @change="modalSelectedToolId = ($event.target as HTMLSelectElement).value ? Number(($event.target as HTMLSelectElement).value) : null; loadModalToolSessions()"
            >
              <option value="">{{ $t('workspaces.default_tool_none') }}</option>
              <option v-for="tool in tools" :key="tool.id" :value="tool.id">{{ tool.name }}</option>
            </select>
          </div>
          <button class="text-button" :disabled="isLoadingModalSessions || !modalSelectedToolId" @click="loadModalToolSessions">
            {{ isLoadingModalSessions ? $t('button.querying') : $t('button.requery') }}
          </button>
        </div>

        <div v-if="modalToolSessions" class="session-modal-body">
          <div class="command-output-heading">
            <span>{{ $t('workspaces.executed_command') }}</span>
            <code>{{ modalToolSessions.command }}</code>
          </div>

          <article v-if="modalToolSessions.sessions.length" class="session-command-list">
            <div v-for="session in modalToolSessions.sessions" :key="session.id" class="session-command-row session-item-row">
              <div>
                <strong>{{ session.title }}</strong>
                <small>{{ session.id }} · {{ session.updated }}</small>
              </div>
              <button class="mini-button" :disabled="isBusy" @click="openModalToolSession(session)">{{ $t('button.open') }}</button>
            </div>
          </article>

          <article v-else-if="modalToolSessions.lines.length" class="session-command-list">
            <div v-for="line in modalToolSessions.lines" :key="line" class="session-command-row">
              {{ line }}
            </div>
          </article>

          <article v-else class="recent-message-card">
            <pre>{{ modalToolSessions.output || modalToolSessions.stderr || $t('workspaces.no_output') }}</pre>
          </article>
        </div>

        <div v-else-if="!isLoadingModalSessions" class="empty-state compact-empty">
          <strong>{{ $t('workspaces.no_sessions_title') }}</strong>
          <p>{{ $t('workspaces.modal_no_sessions_hint') }}</p>
        </div>

        <div v-else class="empty-state compact-empty">
          <p>{{ $t('button.querying') }}</p>
        </div>
      </div>
    </div>
  </main>
</template>
