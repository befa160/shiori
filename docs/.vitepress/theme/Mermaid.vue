<script setup lang="ts">
// Rendu d'un bloc ```mermaid côté navigateur, chargé à la demande, recalculé au changement
// de thème (mermaid n'a pas de thème qui suive le CSS).
import { onMounted, ref, watch } from 'vue'
import { useData } from 'vitepress'

const props = defineProps<{ code: string }>()
const { isDark } = useData()
const svg = ref('')
let count = 0

async function render() {
  const { default: mermaid } = await import('mermaid')
  mermaid.initialize({ startOnLoad: false, theme: isDark.value ? 'dark' : 'neutral', securityLevel: 'strict' })
  const id = `mermaid-${Math.random().toString(36).slice(2)}-${count++}`
  svg.value = (await mermaid.render(id, decodeURIComponent(props.code))).svg
}

onMounted(render)
watch(isDark, render)
</script>

<template>
  <div class="mermaid-diagram" v-html="svg" />
</template>

<style scoped>
.mermaid-diagram {
  margin: 16px 0;
  overflow-x: auto;
  text-align: center;
}
</style>
