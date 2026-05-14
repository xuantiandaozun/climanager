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

type ProxyConfig = {
  enabled: boolean
  host: string
  port: string
}

const workspaces = ref<Workspace[]>([])
const tools = ref<Tool[]>([])
const history = ref<LaunchRecord[]>([])
const sessions = ref<SessionRecord[]>([])
const proxy = ref<ProxyConfig>({ enabled: false, host: '127.0.0.1', port: '7890' })
const selectedWorkspaceId = ref<number | null>(null)
const selectedToolId = ref<number | null>(null)
const statusMessage = ref('准备就绪')
const isBusy = ref(false)

const selectedWorkspace = computed(() =>
  workspaces.value.find((workspace) => workspace.id === selectedWorkspaceId.value),
)

const selectedTool = computed(() => tools.value.find((tool) => tool.id === selectedToolId.value))

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
        <span class="brand-mark">CM</span>
        <div>
          <p>CLI Manager</p>
          <small>workspace memory</small>
        </div>
      </div>

      <nav class="nav-list">
        <a class="active" href="#workspaces">Workspaces</a>
        <a href="#tools">Tools</a>
        <a href="#proxy">Proxy</a>
        <a href="#history">History</a>
      </nav>

      <div class="open-source-card">
        <span>SQLite local-first</span>
        <strong>{{ statusMessage }}</strong>
      </div>
    </aside>

    <section class="content">
      <header class="hero">
        <div>
          <p class="eyebrow">Local-first AI CLI workspace manager</p>
          <h1>项目、CLI 工具、代理环境，一次配置，下次直接继续。</h1>
          <p class="hero-copy">
            工作区、启动记录和会话索引已经接入本地 SQLite；启动终端时会按配置自动注入 HTTP_PROXY、HTTPS_PROXY 和 ALL_PROXY。
          </p>
        </div>
        <div class="hero-actions">
          <button class="primary" :disabled="isBusy" @click="addWorkspace">添加工作区</button>
          <button class="ghost" :disabled="isBusy" @click="scanSessions">扫描会话</button>
          <button class="ghost" :disabled="isBusy" @click="refreshAll">刷新</button>
        </div>
      </header>

      <section id="workspaces" class="panel workspace-panel">
        <div class="panel-heading">
          <div>
            <p class="eyebrow">Pinned projects</p>
            <h2>工作区</h2>
          </div>
          <button class="text-button" :disabled="isBusy" @click="launchSelected">启动所选工具</button>
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

      <section class="split">
        <section id="tools" class="panel">
          <div class="panel-heading compact">
            <div>
              <p class="eyebrow">Launchers</p>
              <h2>工具</h2>
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

        <section id="proxy" class="panel proxy-panel">
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

      <section id="history" class="panel history-panel">
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
  </main>
</template>
