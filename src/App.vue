<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { computed, onMounted, ref } from 'vue'

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
const statusMessage = ref('准备就绪')
const isBusy = ref(false)
const isLoadingToolSessions = ref(false)
const currentPage = ref<PageId>('overview')

const navPages: { id: PageId; label: string; description: string }[] = [
  { id: 'overview', label: '概览', description: '当前配置总览' },
  { id: 'workspaces', label: '工作区', description: '管理项目目录' },
  { id: 'tools', label: '工具', description: '选择 CLI 启动器' },
  { id: 'proxy', label: '代理', description: '终端环境变量' },
  { id: 'records', label: '记录', description: '历史与会话索引' },
]

const selectedWorkspace = computed(() =>
  workspaces.value.find((workspace) => workspace.id === selectedWorkspaceId.value),
)

const selectedTool = computed(() => tools.value.find((tool) => tool.id === selectedToolId.value))

const readyToolCount = computed(() => tools.value.filter((tool) => tool.detected_path).length)

const proxyPreview = computed(() => {
  if (!proxy.value.enabled) return '未启用'
  if (!proxy.value.host || !proxy.value.port) return '代理配置不完整'
  return `http://${proxy.value.host}:${proxy.value.port}`
})

onMounted(() => {
  void refreshAll()
})

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
    statusMessage.value = '本地 SQLite 已同步'
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
      ? `已索引 ${sessions.value.length} 条会话记录`
      : '未扫描到可关联的会话文件'
  } catch (error) {
    statusMessage.value = readableError(error)
  } finally {
    isBusy.value = false
  }
}

async function addWorkspace() {
  const selected = await open({ directory: true, multiple: false, title: '选择项目工作区' })
  if (typeof selected !== 'string') return

  isBusy.value = true
  try {
    workspaces.value = await invoke<Workspace[]>('add_workspace', {
      input: { path: selected },
    })
    selectedWorkspaceId.value = workspaces.value[0]?.id ?? null
    statusMessage.value = '工作区已保存到 SQLite'
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
    statusMessage.value = `已移除 ${workspace.name}`
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
    statusMessage.value = proxy.value.enabled ? `代理已保存：${proxyPreview.value}` : '代理已关闭'
  } catch (error) {
    statusMessage.value = readableError(error)
  } finally {
    isBusy.value = false
  }
}

async function launchSelected() {
  if (!selectedWorkspace.value || !selectedTool.value) {
    statusMessage.value = '请先选择工作区和工具'
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
    statusMessage.value = `已在 ${selectedWorkspace.value.name} 启动 ${selectedTool.value.name}`
    await refreshAll()
  } catch (error) {
    statusMessage.value = readableError(error)
  } finally {
    isBusy.value = false
  }
}

async function loadToolSessions() {
  if (!selectedWorkspace.value || !selectedTool.value) {
    statusMessage.value = '请先选择工作区和工具'
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
      ? `已读取 ${toolSessions.value.lines.length} 条会话记录`
      : '当前工具未返回会话记录'
  } catch (error) {
    statusMessage.value = readableError(error)
  } finally {
    isLoadingToolSessions.value = false
  }
}

async function openToolSession(session: ToolSessionItem) {
  if (!selectedWorkspace.value || !selectedTool.value) {
    statusMessage.value = '请先选择工作区和工具'
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
    statusMessage.value = `已打开会话：${session.title}`
  } catch (error) {
    statusMessage.value = readableError(error)
  } finally {
    isBusy.value = false
  }
}

function formatTime(value: string | null) {
  if (!value) return '未启动'
  return new Intl.DateTimeFormat('zh-CN', {
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
          <small>workspace memory</small>
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
          <span>{{ page.label }}</span>
          <small>{{ page.description }}</small>
        </button>
      </nav>

      <div class="open-source-card">
        <span>SQLite local-first</span>
        <strong>{{ statusMessage }}</strong>
      </div>
    </aside>

    <section class="content">
      <header class="page-header">
        <div>
          <p class="eyebrow">Local-first AI CLI workspace manager</p>
          <h1 v-if="currentPage === 'overview'">概览</h1>
          <h1 v-else-if="currentPage === 'workspaces'">工作区</h1>
          <h1 v-else-if="currentPage === 'tools'">工具</h1>
          <h1 v-else-if="currentPage === 'proxy'">代理配置</h1>
          <h1 v-else>记录</h1>
        </div>
        <div class="hero-actions">
          <button class="ghost" :disabled="isBusy" @click="refreshAll">刷新</button>
        </div>
      </header>

      <section v-if="currentPage === 'overview'" class="page-stack">
        <div class="summary-grid">
          <article class="summary-card">
            <span>工作区</span>
            <strong>{{ workspaces.length }}</strong>
            <button class="text-button" type="button" @click="currentPage = 'workspaces'">管理工作区</button>
          </article>
          <article class="summary-card">
            <span>可用工具</span>
            <strong>{{ readyToolCount }} / {{ tools.length }}</strong>
            <button class="text-button" type="button" @click="currentPage = 'tools'">查看工具</button>
          </article>
          <article class="summary-card">
            <span>代理</span>
            <strong>{{ proxyPreview }}</strong>
            <button class="text-button" type="button" @click="currentPage = 'proxy'">配置代理</button>
          </article>
          <article class="summary-card">
            <span>会话索引</span>
            <strong>{{ sessions.length }}</strong>
            <button class="text-button" type="button" @click="currentPage = 'records'">查看记录</button>
          </article>
        </div>

        <section class="panel launch-panel">
          <div class="panel-heading">
            <div>
              <p class="eyebrow">Quick launch</p>
              <h2>快速启动</h2>
            </div>
            <button class="primary" :disabled="isBusy" @click="launchSelected">启动所选工具</button>
          </div>
          <div class="selection-summary">
            <div class="choice-panel">
              <div class="choice-heading">
                <span>当前工作区</span>
                <strong>{{ selectedWorkspace?.name ?? '未选择' }}</strong>
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
              <p v-else>请先在工作区页面添加一个项目。</p>
            </div>
            <div class="choice-panel">
              <div class="choice-heading">
                <span>当前工具</span>
                <strong>{{ selectedTool?.name ?? '未选择' }}</strong>
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
              <p v-else>暂未加载到可用工具。</p>
            </div>
          </div>
        </section>
      </section>

      <section v-else-if="currentPage === 'workspaces'" class="page-stack">
        <section class="panel workspace-panel">
          <div class="panel-heading">
            <div>
              <p class="eyebrow">Pinned projects</p>
              <h2>管理项目目录</h2>
            </div>
            <div class="panel-actions">
              <button class="ghost" :disabled="isBusy" @click="addWorkspace">添加工作区</button>
              <button class="ghost" :disabled="isLoadingToolSessions" @click="loadToolSessions">
                查看会话列表
              </button>
              <button class="primary" :disabled="isBusy" @click="launchSelected">启动所选工具</button>
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
                <button class="mini-button" @click.stop="removeWorkspace(workspace)">移除</button>
              </div>
            </article>
          </div>

          <div v-else class="empty-state">
            <strong>还没有工作区</strong>
            <p>点击“添加工作区”选择一个项目目录，它会保存到本地 SQLite。</p>
          </div>
        </section>

        <section class="panel recent-message-panel">
          <div class="panel-heading compact">
            <div>
              <p class="eyebrow">Tool sessions</p>
              <h2>会话列表</h2>
              <p class="panel-note">
                当前查询：{{ selectedWorkspace?.name ?? '未选择工作区' }} / {{ selectedTool?.name ?? '未选择工具' }}
              </p>
            </div>
            <button class="text-button" :disabled="isLoadingToolSessions" @click="loadToolSessions">
              {{ isLoadingToolSessions ? '查询中...' : '重新查询' }}
            </button>
          </div>

          <div v-if="toolSessions" class="recent-message-list">
            <div class="command-output-heading">
              <span>执行命令</span>
              <code>{{ toolSessions.command }}</code>
            </div>

            <article v-if="toolSessions.sessions.length" class="session-command-list">
              <div v-for="session in toolSessions.sessions" :key="session.id" class="session-command-row session-item-row">
                <div>
                  <strong>{{ session.title }}</strong>
                  <small>{{ session.id }} · {{ session.updated }}</small>
                </div>
                <button class="mini-button" :disabled="isBusy" @click="openToolSession(session)">打开</button>
              </div>
            </article>

            <article v-else-if="toolSessions.lines.length" class="session-command-list">
              <div v-for="line in toolSessions.lines" :key="line" class="session-command-row">
                {{ line }}
              </div>
            </article>

            <article v-else class="recent-message-card">
              <pre>{{ toolSessions.output || toolSessions.stderr || '命令没有返回内容。' }}</pre>
            </article>
          </div>

          <div v-else class="empty-state compact-empty">
            <strong>暂无会话列表</strong>
            <p>选择工作区和工具后点击“查看会话列表”，会后台执行当前工具的 session 命令，不会保存到数据库。</p>
          </div>
        </section>
      </section>

      <section v-else-if="currentPage === 'tools'" class="page-stack">
        <section class="panel">
          <div class="panel-heading compact">
            <div>
              <p class="eyebrow">Launchers</p>
              <h2>选择默认 CLI 工具</h2>
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
                <p>{{ tool.detected_path ?? '未在 PATH 中检测到，仍可尝试启动或后续改为手动配置路径。' }}</p>
              </div>
              <span :class="['status', tool.detected_path ? 'ready' : 'missing']">
                {{ tool.detected_path ? 'ready' : 'missing' }}
              </span>
            </article>
          </div>
        </section>
      </section>

      <section v-else-if="currentPage === 'proxy'" class="page-stack narrow-page">
        <section class="panel proxy-panel">
          <div class="panel-heading compact">
            <div>
              <p class="eyebrow">Terminal env</p>
              <h2>代理配置</h2>
            </div>
          </div>

          <form class="proxy-form" @submit.prevent="saveProxy">
            <label class="switch-row">
              <input v-model="proxy.enabled" type="checkbox" />
              <span>启动终端时自动设置代理环境变量</span>
            </label>
            <label>
              <span>Host</span>
              <input v-model="proxy.host" placeholder="127.0.0.1" />
            </label>
            <label>
              <span>Port</span>
              <input v-model="proxy.port" placeholder="7890" />
            </label>
            <p>当前：{{ proxyPreview }}</p>
            <button class="primary" :disabled="isBusy">保存代理</button>
          </form>
        </section>
      </section>

      <section v-else class="page-stack">
        <section class="panel history-panel">
          <div class="panel-heading compact">
            <div>
              <p class="eyebrow">Recent memory</p>
              <h2>启动历史</h2>
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
            <strong>暂无启动历史</strong>
            <p>选择工作区和工具后点击启动，会写入 SQLite。</p>
          </div>
        </section>

        <section class="panel history-panel">
          <div class="panel-heading compact">
            <div>
              <p class="eyebrow">Session index</p>
              <h2>会话索引</h2>
            </div>
            <button class="text-button" :disabled="isBusy" @click="scanSessions">重新扫描</button>
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
            <strong>暂无会话索引</strong>
            <p>点击“扫描会话”，会从常见工具历史目录里寻找包含工作区路径或名称的文件。</p>
          </div>
        </section>
      </section>
    </section>
  </main>
</template>
