(() => {
  'use strict';

  // Phosphor CaretRight (16x16)
  const SVG_CARET = `<svg viewBox="0 0 256 256" fill="currentColor" aria-hidden="true"><path d="M181.66,133.66l-80,80a8,8,0,0,1-11.32-11.32L164.69,128,90.34,53.66a8,8,0,0,1,11.32-11.32l80,80A8,8,0,0,1,181.66,133.66Z"/></svg>`;

  // Phosphor File (16x16) for individual file leaf rows
  const SVG_FILE = `<svg viewBox="0 0 256 256" fill="currentColor" aria-hidden="true"><path d="M213.66,82.34l-56-56A8,8,0,0,0,152,24H56A16,16,0,0,0,40,40V216a16,16,0,0,0,16,16H200a16,16,0,0,0,16-16V88A8,8,0,0,0,213.66,82.34ZM160,51.31,188.69,80H160ZM200,216H56V40h88V88a8,8,0,0,0,8,8h48V216Z"/></svg>`;

  // Subtle leaf dot (16x16) for empty or restricted leaf folders
  const SVG_DOT = `<svg viewBox="0 0 256 256" fill="currentColor" aria-hidden="true"><circle cx="128" cy="128" r="24"/></svg>`;

  // Phosphor FolderSimple (16x16)
  const SVG_FOLDER = `<svg viewBox="0 0 256 256" fill="currentColor" aria-hidden="true"><path d="M216,72H130.67L102.93,51.2a16.12,16.12,0,0,0-9.6-3.2H40A16,16,0,0,0,24,64V200a16,16,0,0,0,16,16H216.89A15.13,15.13,0,0,0,232,200.89V88A16,16,0,0,0,216,72Zm0,128H40V64H93.33l27.74,20.8a16.12,16.12,0,0,0,9.6,3.2H216Z"/></svg>`;

  // VS Code ribbon icon (16x16)
  const SVG_VSCODE = `<svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M17.583 2.207a1.5 1.5 0 0 0-1.577.314L7.29 10.487 3.48 7.59a1.002 1.002 0 0 0-1.287.072l-1.39 1.32a1 1 0 0 0-.005 1.442L4.26 13.68.798 16.935a1 1 0 0 0 .005 1.442l1.39 1.32a1.002 1.002 0 0 0 1.287.072l3.81-2.896 8.716 7.966a1.5 1.5 0 0 0 2.51-.955V3.476a1.5 1.5 0 0 0-.933-1.269ZM16.5 17.61l-6.11-5.25 6.11-5.25v10.5Z"/></svg>`;

  // Phosphor Trash (16x16)
  const SVG_TRASH = `<svg viewBox="0 0 256 256" fill="currentColor" aria-hidden="true"><path d="M216,48H176V40a24,24,0,0,0-24-24H104A24,24,0,0,0,80,40v8H40a8,8,0,0,0,0,16h8V208a16,16,0,0,0,16,16H192a16,16,0,0,0,16-16V64h8a8,8,0,0,0,0-16ZM96,40a8,8,0,0,1,8-8h48a8,8,0,0,1,8,8v8H96Zm96,168H64V64H192ZM112,104v64a8,8,0,0,1-16,0V104a8,8,0,0,1,16,0Zm48,0v64a8,8,0,0,1-16,0V104a8,8,0,0,1,16,0Z"/></svg>`;

  const GB = 1024 * 1024 * 1024;
  const MB = 1024 * 1024;
  const KB = 1024;
  const TB = 1024 * GB;

  function formatBytes(bytes) {
    if (bytes >= TB) {
      return `${(bytes / TB).toFixed(2)} TB`;
    }
    if (bytes >= GB) {
      const v = bytes / GB;
      return `${v >= 100 ? v.toFixed(1) : v.toFixed(2)} GB`;
    }
    if (bytes >= MB) {
      const v = bytes / MB;
      return `${v >= 100 ? v.toFixed(0) : v.toFixed(1)} MB`;
    }
    if (bytes >= KB) {
      return `${(bytes / KB).toFixed(0)} KB`;
    }
    return `${bytes} B`;
  }

  function sizeTier(bytes) {
    if (bytes >= 5 * GB) return 'high';
    if (bytes < 50 * MB) return 'low';
    return 'mid';
  }

  const state = {
    activeDrive: 'C:\\',
    drives: [],
    expanded: new Set(),       // set of row keys currently expanded
    childrenCache: new Map(),  // key -> { rows, has_more, remaining_count, remaining_size, next_offset }
    scanning: false,
    leftoversOnly: false,
    orphanCount: 0,
    orphanTotalBytes: 0,
    confirmingKey: null,
    noticeTimer: null,
  };

  const elTitle = document.getElementById('drive-title');
  const elSwitcher = document.getElementById('drive-switcher');
  const elMeta = document.getElementById('drive-meta');
  const elStatus = document.getElementById('scan-status');
  const elLeftovers = document.getElementById('leftovers-btn');
  const elRescan = document.getElementById('rescan-btn');
  const elNotice = document.getElementById('notice-banner');
  const elTreeRoot = document.getElementById('tree-root');

  function showNotice(msg) {
    if (state.noticeTimer) clearTimeout(state.noticeTimer);
    elNotice.textContent = msg;
    elNotice.hidden = false;
    state.noticeTimer = setTimeout(() => {
      elNotice.hidden = true;
    }, 4500);
  }

  function setStatus(text, spinning = false) {
    if (!text) {
      elStatus.textContent = '';
      elStatus.hidden = true;
    } else {
      elStatus.textContent = text;
      elStatus.hidden = false;
    }
    elRescan.dataset.spinning = spinning ? 'true' : 'false';
  }

  async function apiGet(url) {
    const res = await fetch(url, { cache: 'no-store' });
    if (!res.ok) throw new Error(`Request failed (${res.status})`);
    return res.json();
  }

  async function apiPost(url, body) {
    const res = await fetch(url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });
    const data = await res.json().catch(() => ({}));
    if (!res.ok) {
      throw new Error(data.error || `Action failed (${res.status})`);
    }
    return data;
  }

  function renderHeader(drives, activeRoot, orphanCount = 0, orphanTotalBytes = 0) {
    state.drives = drives || [];
    state.activeDrive = activeRoot || state.activeDrive;
    state.orphanCount = orphanCount;
    state.orphanTotalBytes = orphanTotalBytes;

    const current = state.drives.find(
      (d) => d.root.toLowerCase() === state.activeDrive.toLowerCase()
    );

    if (state.drives.length > 1) {
      elTitle.hidden = true;
      elSwitcher.hidden = false;
      elSwitcher.innerHTML = '';
      for (const drv of state.drives) {
        const btn = document.createElement('button');
        btn.type = 'button';
        btn.className = 'drive-pill';
        btn.setAttribute('role', 'radio');
        const active = drv.root.toLowerCase() === state.activeDrive.toLowerCase();
        btn.setAttribute('aria-checked', active ? 'true' : 'false');
        btn.textContent = drv.root;
        btn.addEventListener('click', () => {
          if (!active) switchDrive(drv.root);
        });
        elSwitcher.appendChild(btn);
      }
    } else {
      elTitle.hidden = false;
      elSwitcher.hidden = true;
      elTitle.textContent = state.activeDrive;
    }

    if (current) {
      elMeta.textContent = `${formatBytes(current.used_bytes)} used · ${formatBytes(current.free_bytes)} free`;
    } else {
      elMeta.textContent = '';
    }

    if (state.orphanCount > 0 || state.leftoversOnly) {
      elLeftovers.hidden = false;
      elLeftovers.setAttribute('aria-pressed', state.leftoversOnly ? 'true' : 'false');
      elLeftovers.textContent =
        state.orphanCount > 0
          ? `Leftovers · ${formatBytes(state.orphanTotalBytes)}`
          : 'Leftovers';
    } else {
      elLeftovers.hidden = true;
      elLeftovers.setAttribute('aria-pressed', 'false');
    }
  }

  async function fetchChildrenForKey(key, offset = 0, limit = 200) {
    const params = new URLSearchParams({
      key,
      offset: String(offset),
      limit: String(limit),
    });
    return apiGet(`/api/children?${params.toString()}`);
  }

  function createNodeElement(row, depth) {
    const wrapper = document.createElement('div');
    wrapper.className = 'tree-node';
    wrapper.dataset.key = row.key;

    const treeRow = document.createElement('div');
    treeRow.className = 'tree-row';
    treeRow.dataset.kind = row.kind;
    treeRow.setAttribute('role', 'treeitem');
    const isExpanded = row.expandable && state.expanded.has(row.key);
    if (row.expandable) {
      treeRow.setAttribute('aria-expanded', isExpanded ? 'true' : 'false');
    }

    // 1. Folder / item toggle button
    const toggleBtn = document.createElement('button');
    toggleBtn.type = 'button';
    toggleBtn.className = 'row-toggle';
    toggleBtn.style.setProperty('--indent', `${depth * 16}px`);
    toggleBtn.dataset.expandable = row.expandable ? 'true' : 'false';
    if (row.expandable) {
      toggleBtn.setAttribute('aria-expanded', isExpanded ? 'true' : 'false');
    }
    toggleBtn.title = row.leftover_reason
      ? `${row.path}\n${row.leftover_reason}`
      : row.path;

    const caret = document.createElement('span');
    if (row.expandable) {
      caret.className = 'caret caret-chevron';
      caret.innerHTML = SVG_CARET;
    } else if (row.kind === 'file' || row.kind === 'summary') {
      caret.className = 'caret';
      caret.innerHTML = SVG_FILE;
    } else {
      caret.className = 'caret caret-dot';
      caret.innerHTML = SVG_DOT;
    }

    const nameSpan = document.createElement('span');
    nameSpan.className = 'item-name';
    nameSpan.textContent = row.access_denied ? `${row.name} (restricted)` : row.name;

    toggleBtn.appendChild(caret);
    toggleBtn.appendChild(nameSpan);

    if (row.location_hint) {
      const locSpan = document.createElement('span');
      locSpan.className = 'location-hint';
      locSpan.textContent = row.location_hint;
      toggleBtn.appendChild(locSpan);
    }

    if (row.leftover_reason) {
      const tagSpan = document.createElement('span');
      tagSpan.className = 'leftover-tag';
      tagSpan.title = row.leftover_reason;
      tagSpan.textContent = 'Leftover?';
      toggleBtn.appendChild(tagSpan);
    }

    if (row.expandable) {
      toggleBtn.addEventListener('click', () => toggleExpand(row, depth, wrapper, treeRow, toggleBtn));
    }

    // 2. Right-aligned tabular size
    const sizeSpan = document.createElement('span');
    sizeSpan.className = 'row-size';
    sizeSpan.dataset.tier = sizeTier(row.size);
    sizeSpan.textContent = formatBytes(row.size);

    // 3. Off-to-the-side Folder, VS Code & Recycle Bin action buttons
    const actions = document.createElement('div');
    actions.className = 'row-actions';

    const explorerBtn = document.createElement('button');
    explorerBtn.type = 'button';
    explorerBtn.className = 'icon-btn';
    explorerBtn.setAttribute('aria-label', `Open ${row.name} in File Explorer`);
    explorerBtn.title = 'Open in File Explorer';
    explorerBtn.innerHTML = SVG_FOLDER;
    explorerBtn.addEventListener('click', async (e) => {
      e.stopPropagation();
      try {
        await apiPost('/api/open-explorer', {
          path: row.path,
          select_file: Boolean(row.select_in_explorer),
        });
      } catch (err) {
        showNotice(err.message);
      }
    });

    const vscodeBtn = document.createElement('button');
    vscodeBtn.type = 'button';
    vscodeBtn.className = 'icon-btn';
    vscodeBtn.setAttribute('aria-label', `Open ${row.name} in VS Code`);
    vscodeBtn.title = 'Open in VS Code';
    vscodeBtn.innerHTML = SVG_VSCODE;
    vscodeBtn.addEventListener('click', async (e) => {
      e.stopPropagation();
      try {
        await apiPost('/api/open-vscode', {
          path: row.vscode_path || row.path,
        });
      } catch (err) {
        showNotice(err.message);
      }
    });

    const deleteBtn = document.createElement('button');
    deleteBtn.type = 'button';
    deleteBtn.className = 'icon-btn icon-btn-danger';
    deleteBtn.innerHTML = SVG_TRASH;
    if (!row.can_delete) {
      deleteBtn.disabled = true;
      deleteBtn.setAttribute('aria-label', `Protected item ${row.name}`);
      deleteBtn.title = 'Protected item';
    } else {
      deleteBtn.setAttribute('aria-label', `Move ${row.name} to Recycle Bin`);
      deleteBtn.title = 'Move to Recycle Bin';
      deleteBtn.addEventListener('click', (e) => {
        e.stopPropagation();
        toggleDeleteConfirm(row, wrapper, deleteBtn);
      });
    }

    actions.appendChild(explorerBtn);
    actions.appendChild(vscodeBtn);
    actions.appendChild(deleteBtn);

    treeRow.appendChild(toggleBtn);
    treeRow.appendChild(sizeSpan);
    treeRow.appendChild(actions);
    wrapper.appendChild(treeRow);

    if (row.can_delete && state.confirmingKey === row.key) {
      wrapper.appendChild(buildConfirmRow(row, wrapper, deleteBtn));
    }

    if (isExpanded) {
      const cached = state.childrenCache.get(row.key);
      if (cached) {
        const group = buildChildrenGroup(row.key, cached, depth + 1);
        wrapper.appendChild(group);
      }
    }

    return wrapper;
  }

  function buildChildrenGroup(parentKey, payload, childDepth) {
    const group = document.createElement('div');
    group.className = 'tree-children';
    group.setAttribute('role', 'group');

    for (const childRow of payload.rows) {
      group.appendChild(createNodeElement(childRow, childDepth));
    }

    if (payload.has_more) {
      const moreBtn = document.createElement('button');
      moreBtn.type = 'button';
      moreBtn.className = 'more-btn';
      moreBtn.style.setProperty('--indent', `${childDepth * 16}px`);
      moreBtn.textContent = `Show more (${payload.remaining_count} remaining · ${formatBytes(payload.remaining_size)})`;
      moreBtn.addEventListener('click', async () => {
        moreBtn.disabled = true;
        moreBtn.textContent = 'Loading…';
        try {
          const next = await fetchChildrenForKey(parentKey, payload.next_offset, 200);
          const merged = {
            rows: payload.rows.concat(next.rows),
            has_more: next.has_more,
            remaining_count: next.remaining_count,
            remaining_size: next.remaining_size,
            next_offset: next.next_offset,
          };
          state.childrenCache.set(parentKey, merged);
          const freshGroup = buildChildrenGroup(parentKey, merged, childDepth);
          group.replaceWith(freshGroup);
        } catch (err) {
          moreBtn.disabled = false;
          showNotice(err.message);
        }
      });
      group.appendChild(moreBtn);
    }

    return group;
  }

  async function toggleExpand(row, depth, wrapper, treeRow, toggleBtn) {
    const key = row.key;
    if (state.expanded.has(key)) {
      state.expanded.delete(key);
      treeRow.setAttribute('aria-expanded', 'false');
      toggleBtn.setAttribute('aria-expanded', 'false');
      const existing = wrapper.querySelector(':scope > .tree-children');
      if (existing) existing.remove();
      return;
    }

    state.expanded.add(key);
    treeRow.setAttribute('aria-expanded', 'true');
    toggleBtn.setAttribute('aria-expanded', 'true');

    try {
      let payload = state.childrenCache.get(key);
      if (!payload) {
        payload = await fetchChildrenForKey(key, 0, 200);
        state.childrenCache.set(key, payload);
      }
      if (!state.expanded.has(key)) return;
      const existing = wrapper.querySelector(':scope > .tree-children');
      if (existing) existing.remove();
      wrapper.appendChild(buildChildrenGroup(key, payload, depth + 1));
    } catch (err) {
      state.expanded.delete(key);
      treeRow.setAttribute('aria-expanded', 'false');
      toggleBtn.setAttribute('aria-expanded', 'false');
      showNotice(err.message);
    }
  }

  function closeOpenConfirm() {
    state.confirmingKey = null;
    for (const existing of elTreeRoot.querySelectorAll('.confirm-row')) {
      existing.remove();
    }
  }

  function buildConfirmRow(row, wrapper, deleteBtn) {
    const confirmBar = document.createElement('div');
    confirmBar.className = 'confirm-row';
    confirmBar.setAttribute('role', 'region');
    confirmBar.setAttribute('aria-label', 'Confirm move to Recycle Bin');

    const text = document.createElement('span');
    text.className = 'confirm-text';
    text.textContent = `Move ${row.name} (${formatBytes(row.size)}) to Recycle Bin?`;

    const actionsWrap = document.createElement('div');
    actionsWrap.className = 'confirm-actions';

    const cancelBtn = document.createElement('button');
    cancelBtn.type = 'button';
    cancelBtn.className = 'confirm-cancel-btn';
    cancelBtn.textContent = 'Cancel';
    cancelBtn.addEventListener('click', () => {
      state.confirmingKey = null;
      confirmBar.remove();
      deleteBtn.focus();
    });

    const confirmBtn = document.createElement('button');
    confirmBtn.type = 'button';
    confirmBtn.className = 'confirm-delete-btn';
    confirmBtn.textContent = 'Move to Recycle Bin';
    confirmBtn.addEventListener('click', async () => {
      cancelBtn.disabled = true;
      confirmBtn.disabled = true;
      confirmBtn.textContent = 'Moving…';
      try {
        await apiPost('/api/delete', { path: row.path });
        state.confirmingKey = null;
        const lowerPrefix = row.path.toLowerCase();
        for (const k of Array.from(state.expanded)) {
          if (k.toLowerCase().includes(lowerPrefix)) {
            state.expanded.delete(k);
          }
        }
        for (const k of Array.from(state.childrenCache.keys())) {
          if (k.toLowerCase().includes(lowerPrefix)) {
            state.childrenCache.delete(k);
          }
        }
        showNotice(`Moved ${row.name} to Recycle Bin.`);
        await refreshAllVisibleTrees();
      } catch (err) {
        cancelBtn.disabled = false;
        confirmBtn.disabled = false;
        confirmBtn.textContent = 'Move to Recycle Bin';
        showNotice(err.message);
      }
    });

    actionsWrap.appendChild(cancelBtn);
    actionsWrap.appendChild(confirmBtn);
    confirmBar.appendChild(text);
    confirmBar.appendChild(actionsWrap);
    return confirmBar;
  }

  function toggleDeleteConfirm(row, wrapper, deleteBtn) {
    if (state.confirmingKey === row.key) {
      closeOpenConfirm();
      return;
    }
    closeOpenConfirm();
    state.confirmingKey = row.key;
    const confirmBar = buildConfirmRow(row, wrapper, deleteBtn);
    const treeRow = wrapper.querySelector(':scope > .tree-row');
    if (treeRow && treeRow.nextSibling) {
      wrapper.insertBefore(confirmBar, treeRow.nextSibling);
    } else {
      wrapper.appendChild(confirmBar);
    }
    const cancelBtn = confirmBar.querySelector('.confirm-cancel-btn');
    if (cancelBtn) cancelBtn.focus();
  }

  async function refreshAllVisibleTrees() {
    const data = await apiGet('/api/state');
    renderHeader(data.drives, data.active_drive, data.orphan_count, data.orphan_total_bytes);

    if (data.scanning) {
      state.scanning = true;
      const scannedStr = data.scanned_bytes > 0 ? ` · ${formatBytes(data.scanned_bytes)}` : '';
      setStatus(`Scanning${scannedStr}`, true);
    } else {
      state.scanning = false;
      setStatus('', false);
    }

    if (!data.ready) {
      return;
    }

    // Refresh children for root + every currently expanded folder in parallel
    const expandedKeys = Array.from(state.expanded);
    const rootKey = state.leftoversOnly ? 'orphans' : `dir:${data.active_drive}`;
    const allKeys = [rootKey, ...expandedKeys];

    const results = await Promise.all(
      allKeys.map(async (k) => {
        try {
          const prev = state.childrenCache.get(k);
          const limit = prev ? Math.max(200, prev.rows.length) : 200;
          const res = await fetchChildrenForKey(k, 0, limit);
          return [k, res];
        } catch {
          return [k, null];
        }
      })
    );

    state.childrenCache.clear();
    for (const [k, res] of results) {
      if (res) {
        state.childrenCache.set(k, res);
      } else {
        state.expanded.delete(k);
      }
    }

    const rootPayload = state.childrenCache.get(rootKey);
    elTreeRoot.innerHTML = '';

    if (!rootPayload || rootPayload.rows.length === 0) {
      const empty = document.createElement('div');
      empty.className = 'empty-state';
      empty.textContent = data.scanning
        ? 'Scanning drive folders…'
        : state.leftoversOnly
          ? 'No leftover folders from uninstalled apps detected on this drive.'
          : 'No folders found on this drive.';
      elTreeRoot.appendChild(empty);
      return;
    }

    for (const row of rootPayload.rows) {
      elTreeRoot.appendChild(createNodeElement(row, 0));
    }

    if (rootPayload.has_more) {
      const moreBtn = document.createElement('button');
      moreBtn.type = 'button';
      moreBtn.className = 'more-btn';
      moreBtn.textContent = `Show more (${rootPayload.remaining_count} remaining · ${formatBytes(rootPayload.remaining_size)})`;
      moreBtn.addEventListener('click', async () => {
        const next = await fetchChildrenForKey(rootKey, rootPayload.next_offset, 200);
        rootPayload.rows = rootPayload.rows.concat(next.rows);
        rootPayload.has_more = next.has_more;
        rootPayload.remaining_count = next.remaining_count;
        rootPayload.remaining_size = next.remaining_size;
        rootPayload.next_offset = next.next_offset;
        refreshAllVisibleTrees();
      });
      elTreeRoot.appendChild(moreBtn);
    }
  }

  async function switchDrive(driveRoot) {
    state.expanded.clear();
    state.childrenCache.clear();
    state.confirmingKey = null;
    state.leftoversOnly = false;
    setStatus('Scanning', true);
    try {
      await apiPost('/api/rescan', { drive: driveRoot, force: false });
      await refreshAllVisibleTrees();
    } catch (err) {
      showNotice(err.message);
    }
  }

  elLeftovers.addEventListener('click', () => {
    state.leftoversOnly = !state.leftoversOnly;
    state.confirmingKey = null;
    elLeftovers.setAttribute('aria-pressed', state.leftoversOnly ? 'true' : 'false');
    refreshAllVisibleTrees().catch((err) => showNotice(err.message));
  });

  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && state.confirmingKey) {
      e.preventDefault();
      const key = state.confirmingKey;
      closeOpenConfirm();
      for (const nodeEl of elTreeRoot.querySelectorAll('.tree-node')) {
        if (nodeEl.dataset.key === key) {
          const delBtn = nodeEl.querySelector(':scope > .tree-row .icon-btn-danger');
          if (delBtn && !delBtn.disabled) delBtn.focus();
          break;
        }
      }
    }
  });

  elRescan.addEventListener('click', async () => {
    setStatus('Scanning', true);
    try {
      await apiPost('/api/rescan', { drive: state.activeDrive, force: true });
    } catch (err) {
      setStatus('', false);
      showNotice(err.message);
    }
  });

  // Keyboard navigation across visible tree row toggles
  elTreeRoot.addEventListener('keydown', (e) => {
    const active = document.activeElement;
    if (!active || !active.classList.contains('row-toggle')) return;

    const toggles = Array.from(elTreeRoot.querySelectorAll('.row-toggle'));
    const idx = toggles.indexOf(active);
    if (idx === -1) return;

    if (e.key === 'ArrowDown') {
      e.preventDefault();
      const next = toggles[idx + 1];
      if (next) next.focus();
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      const prev = toggles[idx - 1];
      if (prev) prev.focus();
    } else if (e.key === 'ArrowRight') {
      if (active.dataset.expandable === 'true') {
        e.preventDefault();
        if (active.getAttribute('aria-expanded') !== 'true') {
          active.click();
        } else if (toggles[idx + 1]) {
          toggles[idx + 1].focus();
        }
      }
    } else if (e.key === 'ArrowLeft') {
      e.preventDefault();
      if (active.getAttribute('aria-expanded') === 'true') {
        active.click();
      } else {
        const parentGroup = active.closest('.tree-children');
        if (parentGroup && parentGroup.parentElement) {
          const parentToggle = parentGroup.parentElement.querySelector(':scope > .tree-row > .row-toggle');
          if (parentToggle) parentToggle.focus();
        }
      }
    }
  });

  function connectEvents() {
    const es = new EventSource('/api/events');
    es.addEventListener('progress', (ev) => {
      try {
        const p = JSON.parse(ev.data);
        if (p.scanning) {
          const bytesTxt = p.scanned_bytes > 0 ? ` · ${formatBytes(p.scanned_bytes)}` : '';
          setStatus(`Scanning${bytesTxt}`, true);
        }
      } catch {}
    });
    es.addEventListener('updated', () => {
      refreshAllVisibleTrees().catch(() => {});
    });
    es.onerror = () => {
      es.close();
      setTimeout(connectEvents, 1500);
    };
  }

  refreshAllVisibleTrees().catch((err) => showNotice(err.message));
  connectEvents();
})();
