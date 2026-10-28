/* Cookieless tracker. Wire-compatible with Beacon by ShadowArcanist (MIT).
   Copyright (c) 2026 ShadowArcanist. See NOTICE. */
(() => {
  const script = document.currentScript
  if (!script) return
  const site = script.dataset.site
  if (!site) return
  const domains = (script.dataset.domains || '').split(',').map((d) => d.trim()).filter(Boolean)
  if (domains.length && !domains.includes(location.hostname)) return
  const excludeSearch = script.dataset.excludeSearch === 'true'
  const excludeHash = script.dataset.excludeHash === 'true'
  const endpoint = new URL('/api/receive', script.src)
  const screen = `${window.screen.width}x${window.screen.height}`

  const normalize = (raw) => {
    if (!raw) return raw
    try {
      const u = new URL(raw, location.href)
      if (excludeSearch) u.search = ''
      if (excludeHash) u.hash = ''
      return u.toString()
    } catch {
      return raw
    }
  }
  let currentUrl = normalize(location.href)
  let currentRef = normalize(document.referrer)
  let failureReported = false
  const reportFailure = () => {
    if (failureReported) return
    failureReported = true
    console.info('[Glimwatch] Analytics was disabled automatically because tracking is blocked by the browser or a privacy tool.')
  }
  const payload = () => ({ site, url: currentUrl, referrer: currentRef || null, title: document.title, hostname: location.hostname, screen, language: navigator.language })
  const send = (type, extra) => {
    try {
      const body = JSON.stringify({ type, ...payload(), ...extra })
      fetch(endpoint, { method: 'POST', keepalive: true, credentials: 'omit', headers: { 'content-type': 'application/json' }, body }).catch(reportFailure)
    } catch {
      reportFailure()
    }
  }

  let flushPerformance = () => {}
  const initPerformance = () => {
    let metrics = {}
    let sent = false
    let timeoutId
    let isInitialLoad = true
    let activationStart = 0
    const observe = (type, callback) => {
      try {
        const observer = new PerformanceObserver((list) => list.getEntries().forEach(callback))
        observer.observe({ type, buffered: true })
      } catch {}
    }
    observe('navigation', (entry) => {
      activationStart = entry.activationStart || 0
      metrics.ttfb = Math.max(entry.responseStart - activationStart, 0)
    })
    observe('paint', (entry) => {
      if (entry.name === 'first-contentful-paint') metrics.fcp = Math.max(entry.startTime - activationStart, 0)
    })
    observe('largest-contentful-paint', (entry) => {
      metrics.lcp = Math.max(entry.startTime - activationStart, 0)
    })
    let clsValue = 0
    let clsEntries = []
    observe('layout-shift', (entry) => {
      if (entry.hadRecentInput) return
      const last = clsEntries[clsEntries.length - 1]
      const first = clsEntries[0]
      if (last && entry.startTime - last.startTime - last.duration < 1000 && entry.startTime - first.startTime < 5000) {
        clsValue += entry.value
        clsEntries.push(entry)
      } else {
        clsValue = entry.value
        clsEntries = [entry]
      }
      if (clsValue > (metrics.cls || 0)) metrics.cls = clsValue
    })
    let interactions = {}
    let inpObserver
    const recordInteractions = (entries) => {
      entries.forEach((entry) => {
        if (entry.interactionId && (!interactions[entry.interactionId] || entry.duration > interactions[entry.interactionId])) interactions[entry.interactionId] = entry.duration
      })
    }
    try {
      inpObserver = new PerformanceObserver((list) => recordInteractions(list.getEntries()))
      inpObserver.observe({ type: 'event', buffered: true, durationThreshold: 40 })
    } catch {}
    const computeInp = () => {
      if (inpObserver) recordInteractions(inpObserver.takeRecords())
      const values = Object.values(interactions).sort((a, b) => b - a)
      if (values.length) metrics.inp = values[Math.min(Math.floor(Math.max(values.length, 10) * 0.02), values.length - 1)]
    }
    const entries = (type) => {
      try {
        return performance.getEntriesByType(type) || []
      } catch {
        return []
      }
    }
    const applyFallbacks = () => {
      if (!isInitialLoad) return
      const nav = entries('navigation')[0]
      if (metrics.ttfb === undefined && nav) metrics.ttfb = Math.max(nav.responseStart - (nav.activationStart || 0), 0)
      const fcp = entries('paint').find((e) => e.name === 'first-contentful-paint')
      if (metrics.fcp === undefined && fcp) metrics.fcp = Math.max(fcp.startTime - activationStart, 0)
      const lcps = entries('largest-contentful-paint')
      if (metrics.lcp === undefined && lcps.length) metrics.lcp = Math.max(lcps[lcps.length - 1].startTime - activationStart, 0)
    }
    const sendPerformance = () => {
      if (sent) return
      applyFallbacks()
      computeInp()
      sent = true
      if (timeoutId) clearTimeout(timeoutId)
      if (Object.keys(metrics).length) send('performance', { vitals: metrics })
    }
    flushPerformance = () => {
      sendPerformance()
      isInitialLoad = false
      metrics = {}
      activationStart = 0
      clsValue = 0
      clsEntries = []
      interactions = {}
      sent = false
      if (timeoutId) clearTimeout(timeoutId)
      timeoutId = setTimeout(sendPerformance, 10000)
    }
    timeoutId = setTimeout(sendPerformance, 10000)
    document.addEventListener('visibilitychange', () => {
      if (document.visibilityState === 'hidden') sendPerformance()
    })
    addEventListener('pagehide', sendPerformance)
  }

  const navigated = (url) => {
    const next = normalize(url || location.href)
    if (!next || next === currentUrl) return
    flushPerformance()
    currentRef = currentUrl
    currentUrl = next
    setTimeout(() => send('pageview'), 300)
  }
  for (const name of ['pushState', 'replaceState']) {
    const original = history[name]
    history[name] = function () {
      const result = original.apply(this, arguments)
      navigated(arguments[2])
      return result
    }
  }
  addEventListener('popstate', () => navigated(location.href))

  initPerformance()
  send('pageview')
  const track = (name, data) => send(name, { data })
  window.glimwatchEvent = track
  window.beaconEvent = track
})()
