'use strict';

const $ = (sel, root = document) => root.querySelector(sel);
const $$ = (sel, root = document) => Array.from(root.querySelectorAll(sel));

const EVENT_META = {
  restock: ['🟢', '补货'],
  soldout: ['🔴', '售罄'],
  price_down: ['💰', '降价'],
  price_up: ['⬆️', '涨价'],
  new_product: ['🆕', '新品'],
};

const CHANNEL_LABEL = { local: '本地', telegram: 'Telegram', webhook: 'Webhook' };
const ALERT_STATUS = { local: '本地', sent: '已发送', failed: '失败' };

const state = {
  tab: 'stock',
  status: 'all',
  vendor: '',
  region: '',
  q: '',
  maxPrice: '',
  watchOnly: false,
  sort: 'default',
  editingRuleId: null,
  vendors: [],
  regions: [],
  rules: [],
  summary: null,
};

function esc(value) {
  return String(value ?? '').replace(/[&<>"']/g, (ch) => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
  }[ch]));
}

function safeUrl(value) {
  const url = String(value || '');
  return /^https?:\/\//i.test(url) ? url : '';
}

function fmtTime(iso) {
  if (!iso) return '—';
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return String(iso);
  return d.toLocaleString('zh-CN', {
    month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', hour12: false,
  });
}

function relTime(iso) {
  if (!iso) return '—';
  const t = new Date(iso).getTime();
  if (Number.isNaN(t)) return '—';
  const sec = Math.max(0, Math.floor((Date.now() - t) / 1000));
  if (sec < 60) return `${sec} 秒前`;
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min} 分钟前`;
  const hour = Math.floor(min / 60);
  if (hour < 24) return `${hour} 小时前`;
  return `${Math.floor(hour / 24)} 天前`;
}

async function api(path, { method = 'GET', body } = {}) {
  const init = { method, headers: {} };
  if (body !== undefined) {
    init.headers['Content-Type'] = 'application/json';
    init.body = JSON.stringify(body);
  }
  const resp = await fetch(path, init);
  const text = await resp.text();
  let data = null;
  if (text) {
    try { data = JSON.parse(text); } catch { data = null; }
  }
  if (!resp.ok) {
    throw new Error((data && (data.detail || data.error)) || `HTTP ${resp.status}`);
  }
  return data;
}

let toastTimer = null;
function toast(message, isError = false) {
  const el = $('#toast');
  el.textContent = message;
  el.classList.toggle('err', isError);
  el.classList.add('show');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => el.classList.remove('show'), 3600);
}

async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    const ta = document.createElement('textarea');
    ta.value = text;
    document.body.appendChild(ta);
    ta.select();
    document.execCommand('copy');
    ta.remove();
  }
}

function debounce(fn, wait) {
  let timer = null;
  return (...args) => {
    clearTimeout(timer);
    timer = setTimeout(() => fn(...args), wait);
  };
}

/* ---------- 概要 ---------- */

async function loadSummary() {
  const summary = await api('/api/summary');
  state.summary = summary;
  renderSummary(summary);
}

function renderSummary(s) {
  const c = s.counts;
  $('#stats').innerHTML = `
    <span class="chip">总 <b>${c.total}</b></span>
    <span class="chip ok">有货 <b>${c.ok}</b></span>
    <span class="chip oos">售罄 <b>${c.oos}</b></span>
    <span class="chip watch">关注 <b>${c.watched}</b></span>
    <span class="chip">24h 事件 <b>${c.events_24h}</b></span>
    <span class="chip">24h 告警 <b>${c.alerts_24h}</b></span>
  `;
  $('#tabStockCount').textContent = c.total;

  const live = $('#liveStatus');
  const text = $('#liveText');
  live.classList.remove('ok', 'warn', 'err');
  if (s.last_error) {
    live.classList.add('err');
    text.textContent = `采集异常 · ${s.last_poll ? relTime(s.last_poll.fetched_at) : '未知'}`;
    live.title = s.last_error;
  } else if (!s.last_poll) {
    text.textContent = '等待首次采集';
    live.title = '尚未执行采集';
  } else {
    live.classList.add('ok');
    text.textContent = `采集正常 · ${relTime(s.last_poll.fetched_at)}`;
    live.title = `上次检查：${fmtTime(s.last_poll.fetched_at)}`;
  }
  updateChannelHint();
}

/* ---------- 筛选项 ---------- */

async function loadFilters() {
  const [vendorData, regionData] = await Promise.all([api('/api/vendors'), api('/api/regions')]);
  state.vendors = vendorData.items;
  state.regions = regionData.items;

  const vendorOptions = state.vendors
    .map((v) => `<option value="${esc(v.key)}">${esc(v.name)} (${v.ok}/${v.total})</option>`)
    .join('');
  $('#vendorFilter').innerHTML = `<option value="">全部商家</option>${vendorOptions}`;
  $('#vendorFilter').value = state.vendor;

  $('#regionFilter').innerHTML = `<option value="">全部地区</option>${
    state.regions.map((r) => `<option value="${esc(r.region_cn)}">${esc(r.region_cn)} (${r.ok}/${r.total})</option>`).join('')
  }`;
  $('#regionFilter').value = state.region;

  $('#ruleVendor').innerHTML = `<option value="">全部商家</option>${vendorOptions}`;
}

/* ---------- 库存 ---------- */

async function loadProducts() {
  const params = new URLSearchParams();
  if (state.status !== 'all') params.set('status', state.status);
  if (state.vendor) params.set('vendor', state.vendor);
  if (state.region) params.set('region', state.region);
  if (state.q) params.set('q', state.q);
  if (state.maxPrice !== '') params.set('max_price', state.maxPrice);
  if (state.watchOnly) params.set('watch', '1');
  params.set('sort', state.sort);
  params.set('limit', '1000');

  const data = await api(`/api/products?${params.toString()}`);
  $('#resultCount').textContent = `${data.total} 款套餐`;
  $('#resultHint').textContent = state.watchOnly ? '仅显示已关注套餐' : '价格为源站展示价，美元为参考换算';

  const grid = $('#productGrid');
  grid.innerHTML = data.items.map(cardHtml).join('');
  $('#stockEmpty').classList.toggle('hidden', data.items.length > 0);
}

function cardHtml(p) {
  const logo = /^data:image\//i.test(p.vendor_logo || '')
    ? `<img src="${esc(p.vendor_logo)}" alt="">`
    : `<span class="avatar">${esc((p.vendor_name || '?').slice(0, 1))}</span>`;

  const specEntries = Object.entries(p.spec || {});
  const specs = specEntries.slice(0, 8)
    .map(([key, value]) => `<span class="spec">${esc(key)} ${esc(value)}</span>`)
    .join('') + (specEntries.length > 8 ? `<span class="spec">+${specEntries.length - 8}</span>` : '');

  const priceText = p.price || '—';
  const unit = p.unit ? `<small>/${esc(p.unit)}</small>` : '';
  const usd = (p.price_usd !== null && p.price_usd !== undefined && !String(priceText).includes('$'))
    ? `<em>≈ $${Number(p.price_usd).toFixed(2)}</em>` : '';
  const buyUrl = safeUrl(p.buy_url);
  const buy = buyUrl
    ? `<a class="buy" href="${esc(buyUrl)}" target="_blank" rel="noopener noreferrer">${p.status ? '购买' : '去查看'}</a>`
    : '';
  const series = p.series_name && p.series_name !== p.name
    ? `<div class="series">${esc(p.series_name)}</div>` : '';
  const route = p.route ? `<div class="route"><b>线路</b> ${esc(p.route)}</div>` : '';
  const promo = p.promo
    ? `<span class="promo-line" data-copy="${esc(p.promo)}" title="点击复制优惠码">🎟️ 优惠码 <b>${esc(p.promo)}</b> · 复制</span>`
    : '';

  return `
    <article class="card ${p.status ? '' : 'out'}">
      <div class="card-top">
        <div class="vendor">${logo}<span>${esc(p.vendor_name)}</span></div>
        <div class="badges">
          <span class="flag">${esc(p.region_flag)} ${esc(p.region_cn)}</span>
          <span class="pill ${p.status ? 'in' : 'out'}">${p.status ? '有货' : '售罄'}</span>
        </div>
      </div>
      <div class="pname">${esc(p.name)}</div>
      ${series}
      <div class="specs">${specs}</div>
      ${route}
      ${promo}
      <div class="card-bottom">
        <div class="price">${esc(priceText)}${unit}${usd}</div>
        <div class="card-actions">
          <button class="star ${p.watched ? 'on' : ''}" data-watch="${p.id}" title="${p.watched ? '取消关注' : '加入关注'}">${p.watched ? '★' : '☆'}</button>
          ${buy}
        </div>
      </div>
    </article>
  `;
}

/* ---------- 动态 ---------- */

async function loadEvents() {
  const [eventData, sourceData] = await Promise.all([
    api('/api/events?limit=100'),
    api('/api/source-history?limit=30'),
  ]);

  const eventList = $('#eventList');
  eventList.innerHTML = eventData.items.length
    ? eventData.items.map((e) => {
        const [icon, label] = EVENT_META[e.type] || ['🔔', e.type];
        const price = e.current_price || '';
        const buy = safeUrl(e.buy_url);
        return `
          <div class="event">
            <span class="event-dot ${esc(e.type)}"></span>
            <div class="event-main">
              <div class="event-title">${icon} ${esc(label)} · ${esc(e.vendor_name)} · ${esc(e.product_name)}</div>
              <div class="event-sub">${esc(e.region_cn)} · ${esc(e.series_name)}</div>
            </div>
            <div class="event-price">${esc(price)}</div>
            <div class="event-time">${fmtTime(e.created_at)}</div>
            ${buy ? `<a class="buy" href="${esc(buy)}" target="_blank" rel="noopener noreferrer">查看</a>` : ''}
          </div>
        `;
      }).join('')
    : '<div class="empty">暂无本地事件，等下一次库存变化</div>';

  const sourceList = $('#sourceHistoryList');
  sourceList.innerHTML = sourceData.items.length
    ? sourceData.items.map((h) => {
        const buy = safeUrl(h.buy_url);
        return `
          <div class="event">
            <span class="event-dot new_product"></span>
            <div class="event-main">
              <div class="event-title">${esc(h.name)}</div>
              <div class="event-sub">源站补货记录 · 北京时间</div>
            </div>
            <div class="event-price">${esc(h.price)}</div>
            <div class="event-time">${esc(h.time_text)}</div>
            ${buy ? `<a class="buy" href="${esc(buy)}" target="_blank" rel="noopener noreferrer">查看</a>` : ''}
          </div>
        `;
      }).join('')
    : '<div class="empty">源站暂无补货记录</div>';
}

/* ---------- 告警 ---------- */

async function loadAlerts() {
  const data = await api('/api/alerts?limit=100');
  const tbody = $('#alertTable');
  tbody.innerHTML = data.items.length
    ? data.items.map((a) => `
        <tr>
          <td>${fmtTime(a.sent_at)}</td>
          <td>${esc(a.rule_name || `#${a.rule_id}`)}</td>
          <td>${esc(a.vendor_name)}<span class="muted">${esc(a.product_name)}</span></td>
          <td><span class="tag channel">${esc(CHANNEL_LABEL[a.channel] || a.channel)}</span></td>
          <td><span class="tag ${esc(a.status)}">${esc(ALERT_STATUS[a.status] || a.status)}</span></td>
          <td class="muted">${esc(a.error || '')}</td>
        </tr>
      `).join('')
    : '<tr><td colspan="6" class="muted" style="text-align:center;padding:28px">暂无告警记录</td></tr>';
}

/* ---------- 规则 ---------- */

async function loadRules() {
  const data = await api('/api/rules');
  state.rules = data.items;
  const tbody = $('#ruleTable');
  tbody.innerHTML = state.rules.length
    ? state.rules.map((r) => {
        const events = String(r.event_types || '').split(',')
          .filter(Boolean)
          .map((t) => (EVENT_META[t] || ['', t])[1])
          .join('、');
        return `
          <tr>
            <td><span class="tag ${r.enabled ? 'on' : 'off'}">${r.enabled ? '启用' : '停用'}</span></td>
            <td>${esc(r.name)}<span class="muted">#${r.id}</span></td>
            <td>${esc(events)}</td>
            <td>${esc(ruleCondition(r))}</td>
            <td>${esc(CHANNEL_LABEL[r.channel] || r.channel)}</td>
            <td>${r.cooldown_minutes} 分钟</td>
            <td>
              <button class="btn ghost small" data-rule-edit="${r.id}">编辑</button>
              <button class="btn ghost small" data-rule-toggle="${r.id}">${r.enabled ? '停用' : '启用'}</button>
              <button class="btn danger small" data-rule-delete="${r.id}">删除</button>
            </td>
          </tr>
        `;
      }).join('')
    : '<tr><td colspan="7" class="muted" style="text-align:center;padding:28px">暂无规则</td></tr>';
}

function ruleCondition(rule) {
  const parts = [];
  if (rule.vendor_key) {
    const vendor = state.vendors.find((v) => v.key === rule.vendor_key);
    parts.push(`商家=${vendor ? vendor.name : rule.vendor_key}`);
  }
  if (rule.region_like) parts.push(`地区含「${rule.region_like}」`);
  if (rule.name_like) parts.push(`名称含「${rule.name_like}」`);
  if (rule.route_like) parts.push(`线路含「${rule.route_like}」`);
  if (rule.min_price_usd !== null && rule.min_price_usd !== undefined) parts.push(`≥$${rule.min_price_usd}`);
  if (rule.max_price_usd !== null && rule.max_price_usd !== undefined) parts.push(`≤$${rule.max_price_usd}`);
  if (rule.only_watchlist) parts.push('仅关注');
  return parts.join(' · ') || '全部套餐';
}

function rulePayload(rule, overrides = {}) {
  return {
    name: rule.name,
    enabled: rule.enabled,
    event_types: rule.event_types,
    vendor_key: rule.vendor_key || null,
    region_like: rule.region_like || null,
    name_like: rule.name_like || null,
    route_like: rule.route_like || null,
    min_price_usd: rule.min_price_usd,
    max_price_usd: rule.max_price_usd,
    only_watchlist: rule.only_watchlist,
    channel: rule.channel,
    cooldown_minutes: rule.cooldown_minutes,
    ...overrides,
  };
}

function fillRuleForm(rule) {
  state.editingRuleId = rule.id;
  $('#ruleFormTitle').textContent = `编辑规则 #${rule.id}`;
  $('#ruleName').value = rule.name;
  $('#ruleVendor').value = rule.vendor_key || '';
  $('#ruleRegion').value = rule.region_like || '';
  $('#ruleNameLike').value = rule.name_like || '';
  $('#ruleRoute').value = rule.route_like || '';
  $('#ruleMinPrice').value = rule.min_price_usd ?? '';
  $('#ruleMaxPrice').value = rule.max_price_usd ?? '';
  $('#ruleChannel').value = rule.channel || 'local';
  $('#ruleCooldown').value = rule.cooldown_minutes ?? 30;
  $('#ruleWatchOnly').checked = Boolean(rule.only_watchlist);
  $('#ruleEnabled').checked = Boolean(rule.enabled);
  const types = new Set(String(rule.event_types || '').split(',').filter(Boolean));
  $$('#ruleEvents input').forEach((input) => { input.checked = types.has(input.value); });
  updateChannelHint();
  $('#ruleForm').scrollIntoView({ behavior: 'smooth', block: 'center' });
}

function resetRuleForm() {
  state.editingRuleId = null;
  $('#ruleFormTitle').textContent = '新建规则';
  $('#ruleName').value = '';
  $('#ruleVendor').value = '';
  $('#ruleRegion').value = '';
  $('#ruleNameLike').value = '';
  $('#ruleRoute').value = '';
  $('#ruleMinPrice').value = '';
  $('#ruleMaxPrice').value = '';
  $('#ruleChannel').value = 'local';
  $('#ruleCooldown').value = 30;
  $('#ruleWatchOnly').checked = false;
  $('#ruleEnabled').checked = true;
  $$('#ruleEvents input').forEach((input) => { input.checked = input.value === 'restock'; });
  updateChannelHint();
}

function updateChannelHint() {
  const hint = $('#channelHint');
  if (!hint) return;
  const channel = $('#ruleChannel').value;
  const summary = state.summary || {};
  if (channel === 'telegram') {
    hint.textContent = summary.telegram_configured
      ? '已配置，可直接发送'
      : '未配置：在 config.json 填写 telegram.bot_token 和 chat_id';
  } else if (channel === 'webhook') {
    hint.textContent = summary.webhook_configured
      ? '已配置，可直接发送'
      : '未配置：在 config.json 填写 webhook.url';
  } else {
    hint.textContent = '只写入看板告警记录，不外发';
  }
}

async function submitRule(event) {
  event.preventDefault();
  const eventTypes = $$('#ruleEvents input:checked').map((input) => input.value);
  if (!eventTypes.length) {
    toast('至少选择一个事件类型', true);
    return;
  }
  const name = $('#ruleName').value.trim();
  if (!name) {
    toast('请填写规则名称', true);
    return;
  }
  const numberOrNull = (value) => (value === '' ? null : Number(value));
  const payload = {
    name,
    enabled: $('#ruleEnabled').checked,
    event_types: eventTypes.join(','),
    vendor_key: $('#ruleVendor').value || null,
    region_like: $('#ruleRegion').value.trim() || null,
    name_like: $('#ruleNameLike').value.trim() || null,
    route_like: $('#ruleRoute').value.trim() || null,
    min_price_usd: numberOrNull($('#ruleMinPrice').value),
    max_price_usd: numberOrNull($('#ruleMaxPrice').value),
    only_watchlist: $('#ruleWatchOnly').checked,
    channel: $('#ruleChannel').value,
    cooldown_minutes: Number($('#ruleCooldown').value || 30),
  };

  try {
    if (state.editingRuleId) {
      await api(`/api/rules/${state.editingRuleId}`, { method: 'PUT', body: payload });
      toast('规则已更新');
    } else {
      await api('/api/rules', { method: 'POST', body: payload });
      toast('规则已创建');
    }
    resetRuleForm();
    await loadRules();
  } catch (error) {
    toast(error.message, true);
  }
}

/* ---------- 采集日志 ---------- */

async function loadPolls() {
  const data = await api('/api/polls?limit=60');
  const tbody = $('#pollTable');
  tbody.innerHTML = data.items.length
    ? data.items.map((p) => `
        <tr>
          <td>${fmtTime(p.fetched_at)}</td>
          <td>${p.http_status ?? '—'}</td>
          <td>${p.changed ? '<span class="tag on">有变化</span>' : '<span class="tag off">无变化</span>'}</td>
          <td>${p.total ?? '—'}</td>
          <td>${p.ok ?? '—'}</td>
          <td>${p.oos ?? '—'}</td>
          <td>${p.duration_ms ?? '—'} ms</td>
          <td class="muted">${esc(p.error || '')}</td>
        </tr>
      `).join('')
    : '<tr><td colspan="8" class="muted" style="text-align:center;padding:28px">暂无采集记录</td></tr>';
}

/* ---------- Tab 与刷新 ---------- */

function switchTab(tab) {
  state.tab = tab;
  $$('#tabs button').forEach((button) => {
    button.classList.toggle('active', button.dataset.tab === tab);
  });
  $$('.panel').forEach((panel) => panel.classList.add('hidden'));
  $(`#panel-${tab}`).classList.remove('hidden');

  if (tab === 'stock') loadProducts().catch((e) => toast(e.message, true));
  else if (tab === 'events') loadEvents().catch((e) => toast(e.message, true));
  else if (tab === 'alerts') loadAlerts().catch((e) => toast(e.message, true));
  else if (tab === 'rules') loadRules().catch((e) => toast(e.message, true));
  else if (tab === 'logs') loadPolls().catch((e) => toast(e.message, true));
}

async function refreshCurrentTab() {
  if (state.tab === 'stock') await loadProducts();
  else if (state.tab === 'events') await loadEvents();
  else if (state.tab === 'alerts') await loadAlerts();
  else if (state.tab === 'rules') await loadRules();
  else if (state.tab === 'logs') await loadPolls();
}

async function refreshAll() {
  await loadSummary();
  await refreshCurrentTab();
}

async function collectNow() {
  const button = $('#collectBtn');
  button.disabled = true;
  button.textContent = '采集中…';
  try {
    const result = await api('/api/collect', { method: 'POST' });
    if (result.skipped) {
      toast('已有采集任务在进行中');
    } else if (!result.ok) {
      toast(`采集失败：${result.error}`, true);
    } else if (result.not_modified) {
      toast('源站数据未变化（304）');
    } else if (result.changed) {
      toast(`采集完成：事件 ${result.events.length} 条，告警 ${result.alerts.length} 条`);
    } else {
      toast(`采集完成：无变化（${result.ok_count} 有货 / ${result.oos_count} 售罄）`);
    }
    await loadSummary();
    await refreshCurrentTab();
  } catch (error) {
    toast(error.message, true);
  } finally {
    button.disabled = false;
    button.textContent = '立即采集';
  }
}

/* ---------- 事件绑定 ---------- */

function bindEvents() {
  $('#tabs').addEventListener('click', (event) => {
    const button = event.target.closest('button[data-tab]');
    if (button) switchTab(button.dataset.tab);
  });

  $('#refreshBtn').addEventListener('click', () => {
    refreshAll().catch((error) => toast(error.message, true));
  });
  $('#collectBtn').addEventListener('click', collectNow);

  $('#q').addEventListener('input', debounce((event) => {
    state.q = event.target.value.trim();
    loadProducts().catch((error) => toast(error.message, true));
  }, 280));

  $('#vendorFilter').addEventListener('change', (event) => {
    state.vendor = event.target.value;
    loadProducts().catch((error) => toast(error.message, true));
  });
  $('#regionFilter').addEventListener('change', (event) => {
    state.region = event.target.value;
    loadProducts().catch((error) => toast(error.message, true));
  });
  $('#maxPrice').addEventListener('input', debounce((event) => {
    state.maxPrice = event.target.value;
    loadProducts().catch((error) => toast(error.message, true));
  }, 400));
  $('#watchOnly').addEventListener('change', (event) => {
    state.watchOnly = event.target.checked;
    loadProducts().catch((error) => toast(error.message, true));
  });
  $('#sortSelect').addEventListener('change', (event) => {
    state.sort = event.target.value;
    loadProducts().catch((error) => toast(error.message, true));
  });
  $('#statusSeg').addEventListener('click', (event) => {
    const button = event.target.closest('button[data-status]');
    if (!button) return;
    state.status = button.dataset.status;
    $$('#statusSeg button').forEach((item) => item.classList.toggle('active', item === button));
    loadProducts().catch((error) => toast(error.message, true));
  });

  $('#productGrid').addEventListener('click', async (event) => {
    const star = event.target.closest('[data-watch]');
    if (star) {
      try {
        const result = await api(`/api/watch/${star.dataset.watch}`, { method: 'POST' });
        star.classList.toggle('on', result.watched);
        star.textContent = result.watched ? '★' : '☆';
        toast(result.watched ? '已加入关注' : '已取消关注');
        await loadSummary();
        if (state.watchOnly) await loadProducts();
      } catch (error) {
        toast(error.message, true);
      }
      return;
    }
    const copy = event.target.closest('[data-copy]');
    if (copy) {
      await copyText(copy.dataset.copy);
      toast(`已复制优惠码：${copy.dataset.copy}`);
    }
  });

  $('#ruleForm').addEventListener('submit', submitRule);
  $('#ruleCancel').addEventListener('click', resetRuleForm);
  $('#ruleChannel').addEventListener('change', updateChannelHint);

  $('#ruleTable').addEventListener('click', async (event) => {
    const edit = event.target.closest('[data-rule-edit]');
    const toggle = event.target.closest('[data-rule-toggle]');
    const remove = event.target.closest('[data-rule-delete]');

    if (edit) {
      const rule = state.rules.find((r) => r.id === Number(edit.dataset.ruleEdit));
      if (rule) fillRuleForm(rule);
      return;
    }
    if (toggle) {
      const rule = state.rules.find((r) => r.id === Number(toggle.dataset.ruleToggle));
      if (!rule) return;
      try {
        await api(`/api/rules/${rule.id}`, {
          method: 'PUT',
          body: rulePayload(rule, { enabled: !rule.enabled }),
        });
        toast(rule.enabled ? '规则已停用' : '规则已启用');
        await loadRules();
      } catch (error) {
        toast(error.message, true);
      }
      return;
    }
    if (remove) {
      const rule = state.rules.find((r) => r.id === Number(remove.dataset.ruleDelete));
      if (!rule) return;
      if (!window.confirm(`删除规则「${rule.name}」？`)) return;
      try {
        await api(`/api/rules/${rule.id}`, { method: 'DELETE' });
        toast('规则已删除');
        if (state.editingRuleId === rule.id) resetRuleForm();
        await loadRules();
      } catch (error) {
        toast(error.message, true);
      }
    }
  });
}

async function init() {
  bindEvents();
  try {
    await Promise.all([loadSummary(), loadFilters()]);
    await loadProducts();
  } catch (error) {
    toast(error.message, true);
  }
  setInterval(() => {
    if (document.hidden) return;
    refreshAll().catch(() => {});
  }, 60000);
}

init();
