import { createApp } from 'vue'
import { createI18n } from 'vue-i18n'
import './style.css'
import App from './App.vue'
import zhCN from './locales/zh-CN.json'
import en from './locales/en.json'

const i18n = createI18n({
  locale: navigator.language.startsWith('zh') ? 'zh-CN' : 'en',
  fallbackLocale: 'en',
  messages: { 'zh-CN': zhCN, en },
})

const app = createApp(App)
app.use(i18n)
app.mount('#app')
