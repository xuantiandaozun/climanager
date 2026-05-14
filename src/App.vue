<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { computed, onMounted, ref } from 'vue'
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
}

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
const toolSessions = ref<ToolSessionList | null>(null)
const proxy = ref<ProxyConfig>({ enabled: false, host: '127.0.0.1', port: '7890' })
const selectedWorkspaceId = ref<number | null>(null)
const selectedToolId = ref<number | null>(null)
const statusMessage = ref('')
const isBusy = ref(false)
const isLoadingToolSessions = ref(false)
const currentPage = ref<PageId>('overview')

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

onMounted(() => {
  statusMessage.value = t('status.ready')
  void refreshAll()
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
    selectedWorkspaceId.value ??= workspaces.value[0]?.id ?? null
    selectedToolId.value ??= tools.value[0]?.id ?? null
    statusMessage.value = t('status.synced')
  } catch (error) {
    statusMessage.value = readableError(error)
  } finally {
    isBusy.value = false
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
    selectedWorkspaceId.value = workspaces.value[0]?.id ?? null
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
    await refreshAll()
  } catch (error) {
    statusMessage.value = readableError(error)
  } finally {
    isBusy.value = false
  }
}

async function loadToolSessions() {
  if (!selectedWorkspace.value || !selectedTool.value) {
    statusMessage.value = t('status.select_workspace_and_tool')
    return
  }

  isLoadingToolSessions.value = true
  try {
    toolSessions.value = await invoke<ToolSessionList>('list_tool_sessions', {
      input: {
        workspace_id: selectedWorkspace.value.id,
        tool_id: selectedTool.value.id,
      },
    })
    statusMessage.value = toolSessions.value.lines.length
      ? t('status.read_session_records', { count: toolSessions.value.lines.length })
      : t('status.no_session_records')
  } catch (error) {
    statusMessage.value = readableError(error)
  } finally {
    isLoadingToolSessions.value = false
  }
}

async function openToolSession(session: ToolSessionItem) {
  if (!selectedWorkspace.value || !selectedTool.value) {
    statusMessage.value = t('status.select_workspace_and_tool')
    return
  }

  isBusy.value = true
  try {
    await invoke('open_tool_session', {
      input: {
        workspace_id: selectedWorkspace.value.id,
        tool_id: selectedTool.value.id,
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
          <div class="panel-heading">
            <div>
              <p class="eyebrow">{{ $t('launch.quick_launch') }}</p>
              <h2>{{ $t('launch.quick_launch') }}</h2>
            </div>
            <button class="primary" :disabled="isBusy" @click="launchSelected">{{ $t('button.launch_selected') }}</button>
          </div>
          <div class="selection-summary">
            <div class="choice-panel">
              <div class="choice-heading">
                <span>{{ $t('launch.current_workspace') }}</span>
                <strong>{{ selectedWorkspace?.name ?? $t('launch.not_selected') }}</strong>
              </div>
              <div v-if="workspaces.length" class="choice-list">
                <button
                  v-for="workspace in workspaces"
                  :key="workspace.id"
                  :class="['choice-row', { selected: workspace.id === selectedWorkspaceId }]"
                  type="button"
                  @click="selectedWorkspaceId = workspace.id"
                >
                  <span>{{ workspace.name }}</span>
                  <small>{{ workspace.path }}</small>
                </button>
              </div>
              <p v-else>{{ $t('launch.no_workspaces_hint') }}</p>
            </div>
            <div class="choice-panel">
              <div class="choice-heading">
                <span>{{ $t('launch.current_tool') }}</span>
                <strong>{{ selectedTool?.name ?? $t('launch.not_selected') }}</strong>
              </div>
              <div v-if="tools.length" class="choice-list">
                <button
                  v-for="tool in tools"
                  :key="tool.id"
                  :class="['choice-row', { selected: tool.id === selectedToolId }]"
                  type="button"
                  @click="selectedToolId = tool.id"
                >
                  <span>{{ tool.name }}</span>
                  <small>{{ tool.command }}</small>
                </button>
              </div>
              <p v-else>{{ $t('launch.no_tools_hint') }}</p>
            </div>
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
              <button class="ghost" :disabled="isLoadingToolSessions" @click="loadToolSessions">
                {{ $t('button.view_sessions') }}
              </button>
              <button class="primary" :disabled="isBusy" @click="launchSelected">{{ $t('button.launch_selected') }}</button>
            </div>
          </div>

          <div v-if="workspaces.length" class="workspace-grid">
            <article
              v-for="workspace in workspaces"
              :key="workspace.id"
              :class="['workspace-card', { selected: workspace.id === selectedWorkspaceId }]"
              @click="selectedWorkspaceId = workspace.id"
            >
              <div class="workspace-topline">
                <span class="folder-dot"></span>
                <span>{{ formatTime(workspace.last_opened_at) }}</span>
              </div>
              <h3>{{ workspace.name }}</h3>
              <p>{{ workspace.path }}</p>
              <div class="workspace-meta">
                <span>ID {{ workspace.id }}</span>
                <div class="workspace-actions">
                  <button class="mini-button" :disabled="isBusy" @click.stop="openWorkspaceInVSCode(workspace)">
                    {{ $t('button.open_in_vscode_short') }}
                  </button>
                  <button class="mini-button" :disabled="isBusy" @click.stop="removeWorkspace(workspace)">
                    {{ $t('button.remove') }}
                  </button>
                </div>
              </div>
            </article>
          </div>

          <div v-else class="empty-state">
            <strong>{{ $t('workspaces.no_workspaces_title') }}</strong>
            <p>{{ $t('workspaces.no_workspaces_hint') }}</p>
          </div>
        </section>

        <section class="panel recent-message-panel">
          <div class="panel-heading compact">
            <div>
              <p class="eyebrow">{{ $t('workspaces.session_list') }}</p>
              <h2>{{ $t('workspaces.session_list') }}</h2>
              <p class="panel-note">
                {{ $t('workspaces.current_query', { workspace: selectedWorkspace?.name ?? $t('launch.not_selected'), tool: selectedTool?.name ?? $t('launch.not_selected') }) }}
              </p>
            </div>
            <button class="text-button" :disabled="isLoadingToolSessions" @click="loadToolSessions">
              {{ isLoadingToolSessions ? $t('button.querying') : $t('button.requery') }}
            </button>
          </div>

          <div v-if="toolSessions" class="recent-message-list">
            <div class="command-output-heading">
              <span>{{ $t('workspaces.executed_command') }}</span>
              <code>{{ toolSessions.command }}</code>
            </div>

            <article v-if="toolSessions.sessions.length" class="session-command-list">
              <div v-for="session in toolSessions.sessions" :key="session.id" class="session-command-row session-item-row">
                <div>
                  <strong>{{ session.title }}</strong>
                  <small>{{ session.id }} · {{ session.updated }}</small>
                </div>
                <button class="mini-button" :disabled="isBusy" @click="openToolSession(session)">{{ $t('button.open') }}</button>
              </div>
            </article>

            <article v-else-if="toolSessions.lines.length" class="session-command-list">
              <div v-for="line in toolSessions.lines" :key="line" class="session-command-row">
                {{ line }}
              </div>
            </article>

            <article v-else class="recent-message-card">
              <pre>{{ toolSessions.output || toolSessions.stderr || $t('workspaces.no_output') }}</pre>
            </article>
          </div>

          <div v-else class="empty-state compact-empty">
            <strong>{{ $t('workspaces.no_sessions_title') }}</strong>
            <p>{{ $t('workspaces.no_sessions_hint') }}</p>
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
  </main>
</template>
