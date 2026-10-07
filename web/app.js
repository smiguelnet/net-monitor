'use strict';

const POLL_INTERVAL_MS = 2000;

const STATUS_LABELS = {
  ok: 'Operational',
  degraded: 'Degraded',
  down: 'Down',
  unconfigured: 'Not configured',
};

const BANNER_ICONS = { error: '!', warning: '!', info: 'i', ok: '✓' };

const GROUPS = [
  { id: 'internet', title: d => (d.isp_name ? `Internet · ${d.isp_name}` : 'Internet') },
  { id: 'local', title: d => (d.router_name ? `Local network · ${d.router_name}` : 'Local network') },
  { id: 'access_points', title: () => 'Access points' },
];

const app = {
  data: null,
  error: null,
  clockOffsetMs: 0,      // browser clock minus server clock
  bannerKey: '',
  topologyKey: '',
  cards: new Map(),      // check id -> { root, refs }
};

// ---------- DOM helpers ----------

const $ = id => document.getElementById(id);

/** el('div', { class: 'x', text: 'y', title: 'z' }, ...children) */
function el(tag, props = {}, ...children) {
  const node = document.createElement(tag);
  for (const [key, value] of Object.entries(props)) {
    if (value == null) continue;
    if (key === 'class') node.className = value;
    else if (key === 'text') node.textContent = value;
    else node.setAttribute(key, value);
  }
  node.append(...children);
  return node;
}

const serverNow = () => Date.now() - app.clockOffsetMs;

function formatLatency(ms) {
  if (ms == null) return '—';
  return ms < 100 ? ms.toFixed(1) : String(Math.round(ms));
}

// ---------- Banners ----------

function renderBanners() {
  const d = app.data;
  const items = [];
  if (app.error) {
    items.push({ level: 'error', message: `The monitor is not responding (${app.error}). Make sure it is still running.` });
  } else if (d && !d.ready) {
    items.push({ level: 'info', message: 'Running the first check…' });
  }
  if (d && d.ready) {
    const ageS = (serverNow() - d.checked_at_ms) / 1000;
    if (!d.checking && ageS > d.interval_s * 3 + 15) {
      items.push({ level: 'warning', message: 'Data is stale: no check has completed recently.' });
    }
    items.push(...d.banners);
  }

  const key = JSON.stringify(items);
  if (key === app.bannerKey) return; // avoid replaying the entrance animation
  app.bannerKey = key;
  $('banners').replaceChildren(...items.map(b =>
    el('div', { class: `banner banner-${b.level}`, role: b.level === 'error' ? 'alert' : null },
      el('span', { class: 'icon', text: BANNER_ICONS[b.level] || 'i' }),
      el('span', { text: b.message }))));
}

// ---------- Topology ----------

function topologyNode(n) {
  return el('div', { class: `node is-${n.status}`, title: STATUS_LABELS[n.status] },
    el('span', { class: 'dot' }),
    el('div', {},
      el('div', { class: 'node-name', text: n.name }),
      el('div', { class: 'node-target', text: n.target })));
}

function renderTopology(d) {
  const key = JSON.stringify(d.nodes);
  if (key === app.topologyKey) return;
  app.topologyKey = key;

  const internet = d.nodes.find(n => n.kind === 'internet');
  const router = d.nodes.find(n => n.kind === 'router');
  const aps = d.nodes.filter(n => n.kind === 'ap');
  const link = status => el('span', { class: `link is-${status}` });

  const parts = [topologyNode(internet), link(internet.status), topologyNode(router)];
  if (aps.length) {
    parts.push(link(router.status), el('div', { class: 'ap-stack' }, ...aps.map(topologyNode)));
  }
  $('topology').replaceChildren(...parts);
}

// ---------- Check cards ----------

function createCard() {
  const refs = {};
  const ref = (name, node) => (refs[name] = node);
  const root = el('article', { class: 'card' },
    el('div', { class: 'card-head' },
      el('div', {},
        ref('title', el('h3', { class: 'card-title' })),
        ref('target', el('p', { class: 'card-target' }))),
      el('span', { class: 'badge' }, el('span', { class: 'dot' }), ref('badge', el('span')))),
    el('div', { class: 'metrics' },
      el('div', { class: 'metric' },
        el('span', { class: 'metric-label', text: 'Latency' }),
        ref('latency', el('span', { class: 'metric-value' }))),
      el('div', { class: 'metric' },
        el('span', { class: 'metric-label', text: 'Packet loss' }),
        ref('loss', el('span', { class: 'metric-value' })))),
    el('div', { class: 'meter', title: 'Latency relative to the warning threshold' },
      ref('meter', el('div', { class: 'meter-fill' }))),
    ref('detail', el('p', { class: 'card-detail' })));
  return { root, refs };
}

function setMetric(node, value, unit) {
  node.replaceChildren(value, ...(value === '—' ? [] : [el('small', { text: unit })]));
}

function updateCard({ root, refs }, check, warningMs) {
  root.className = `card is-${check.status}`;
  refs.title.textContent = check.name;
  refs.target.textContent = check.target || '—';
  refs.badge.textContent = STATUS_LABELS[check.status] || check.status;
  setMetric(refs.latency, formatLatency(check.latency_ms), 'ms');
  setMetric(refs.loss, check.loss_pct == null ? '—' : String(check.loss_pct), '%');
  const ratio = check.latency_ms == null ? 0 : Math.max(Math.min(check.latency_ms / warningMs, 1), 0.03);
  refs.meter.style.width = `${ratio * 100}%`;
  refs.detail.textContent = check.detail || '';
}

function groupSection(container, group) {
  let section = container.querySelector(`[data-group="${group.id}"]`);
  if (!section) {
    section = el('section', { class: 'group', 'data-group': group.id },
      el('h2', { class: 'group-title' }, el('span'), el('span', { class: 'group-count' })),
      el('div', { class: 'grid' }));
    container.append(section);
  }
  return section;
}

/** Updates cards in place (keyed by check id) so they don't flicker on refresh. */
function renderGroups(d) {
  const container = $('groups');
  container.querySelectorAll('.skeleton-group').forEach(n => n.remove());
  const seen = new Set();

  for (const group of GROUPS) {
    const checks = d.checks.filter(c => c.group === group.id);
    if (!checks.length) {
      container.querySelector(`[data-group="${group.id}"]`)?.remove();
      continue;
    }
    const section = groupSection(container, group);
    section.dataset.span = Math.min(checks.length, 4); // columns used in the 4-column layout
    const [title, count] = section.querySelector('.group-title').children;
    title.textContent = group.title(d);
    count.textContent = `${checks.filter(c => c.status === 'ok').length}/${checks.length} healthy`;

    const grid = section.querySelector('.grid');
    checks.forEach((check, i) => {
      let card = app.cards.get(check.id);
      if (!card) {
        card = createCard();
        app.cards.set(check.id, card);
      }
      if (grid.children[i] !== card.root) grid.insertBefore(card.root, grid.children[i] || null);
      updateCard(card, check, d.latency_warning_ms);
      seen.add(check.id);
    });
  }

  for (const [id, card] of app.cards) {
    if (!seen.has(id)) {
      card.root.remove();
      app.cards.delete(id);
    }
  }
}

// ---------- Refresh indicator ----------

/** Runs several times per second: progress bar fills until the next check. */
function tick() {
  const bar = $('progress');
  const fill = $('progress-bar');
  const pill = $('refresh-pill');
  const text = $('refresh-text');
  const d = app.data;

  if (app.error) {
    bar.classList.remove('indeterminate');
    fill.style.width = '0%';
    pill.classList.add('busy');
    text.textContent = 'Disconnected — retrying…';
    return;
  }
  if (!d || !d.ready || d.checking) {
    bar.classList.add('indeterminate');
    pill.classList.add('busy');
    text.textContent = d && d.ready ? 'Checking now…' : 'Running first check…';
    return;
  }

  bar.classList.remove('indeterminate');
  pill.classList.remove('busy');
  const elapsedMs = serverNow() - d.checked_at_ms;
  const totalMs = d.interval_s * 1000;
  fill.style.width = `${Math.min(elapsedMs / totalMs, 1) * 100}%`;
  const nextS = Math.max(0, Math.ceil((totalMs - elapsedMs) / 1000));
  const updated = new Date(d.checked_at_ms + app.clockOffsetMs).toLocaleTimeString();
  text.textContent = `Updated ${updated} · next check in ${nextS}s`;
}

// ---------- Polling ----------

async function poll() {
  try {
    const res = await fetch('/api/status', { cache: 'no-store' });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const d = await res.json();
    app.clockOffsetMs = Date.now() - d.server_time_ms;
    app.data = d;
    app.error = null;
    if (d.ready) {
      renderTopology(d);
      renderGroups(d);
    }
  } catch (e) {
    app.error = e.message || String(e);
  } finally {
    renderBanners();
    tick();
    setTimeout(poll, POLL_INTERVAL_MS);
  }
}

poll();
setInterval(tick, 250);
