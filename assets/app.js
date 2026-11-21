(() => {
  'use strict';

  // Phosphor CaretRight (16x16)
  const SVG_CARET = `<svg viewBox="0 0 256 256" fill="currentColor" aria-hidden="true"><path d="M181.66,133.66l-80,80a8,8,0,0,1-11.32-11.32L164.69,128,90.34,53.66a8,8,0,0,1,11.32-11.32l80,80A8,8,0,0,1,181.66,133.66Z"/></svg>`;

  // Phosphor ArrowLeft (16x16)
  const SVG_BACK = `<svg viewBox="0 0 256 256" fill="currentColor" aria-hidden="true"><path d="M224,128a8,8,0,0,1-8,8H59.31l58.35,58.34a8,8,0,0,1-11.32,11.32l-72-72a8,8,0,0,1,0-11.32l72-72a8,8,0,0,1,11.32,11.32L59.31,120H216A8,8,0,0,1,224,128Z"/></svg>`;

  // Phosphor File (16x16) for individual file leaf rows
  const SVG_FILE = `<svg viewBox="0 0 256 256" fill="currentColor" aria-hidden="true"><path d="M213.66,82.34l-56-56A8,8,0,0,0,152,24H56A16,16,0,0,0,40,40V216a16,16,0,0,0,16,16H200a16,16,0,0,0,16-16V88A8,8,0,0,0,213.66,82.34ZM160,51.31,188.69,80H160ZM200,216H56V40h88V88a8,8,0,0,0,8,8h48V216Z"/></svg>`;

  // Subtle leaf dot (16x16) for empty or restricted leaf folders
  const SVG_DOT = `<svg viewBox="0 0 256 256" fill="currentColor" aria-hidden="true"><circle cx="128" cy="128" r="24"/></svg>`;

  // Phosphor FolderSimple (16x16)
  const SVG_FOLDER = `<svg viewBox="0 0 256 256" fill="currentColor" aria-hidden="true"><path d="M216,72H130.67L102.93,51.2a16.12,16.12,0,0,0-9.6-3.2H40A16,16,0,0,0,24,64V200a16,16,0,0,0,16,16H216.89A15.13,15.13,0,0,0,232,200.89V88A16,16,0,0,0,216,72Zm0,128H40V64H93.33l27.74,20.8a16.12,16.12,0,0,0,9.6,3.2H216Z"/></svg>`;

  // VS Code ribbon icon (16x16)
  const SVG_VSCODE = `<svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M17.583 2.207a1.5 1.5 0 0 0-1.577.314L7.29 10.487 3.48 7.59a1.002 1.002 0 0 0-1.287.072l-1.39 1.32a1 1 0 0 0-.005 1.442L4.26 13.68.798 16.935a1 1 0 0 0 .005 1.442l1.39 1.32a1.002 1.002 0 0 0 1.287.072l3.81-2.896 8.716 7.966a1.5 1.5 0 0 0 2.51-.955V3.476a1.5 1.5 0 0 0-.933-1.269ZM16.5 17.61l-6.11-5.25 6.11-5.25v10.5Z"/></svg>`;

  // Phosphor EyeSlash (16x16) for "Keep / Not a leftover"
  const SVG_KEEP = `<svg viewBox="0 0 256 256" fill="currentColor" aria-hidden="true"><path d="M53.92,34.62A8,8,0,1,0,42.08,45.38L61.32,66.55C25,88.84,9.38,123.2,8.69,124.76a8,8,0,0,0,0,6.5c.35.79,8.82,19.57,27.65,38.4C61.43,194.74,93.12,208,128,208a127.11,127.11,0,0,0,52.07-10.83l22,24.21a8,8,0,1,0,11.84-10.76Zm47.33,75.84,41.67,45.85a32,32,0,0,1-41.67-45.85ZM128,192c-30.78,0-57.67-11.19-79.93-33.25A133.16,133.16,0,0,1,25,128c4.69-8.79,19.66-33.39,47.35-49.38l18,19.75a48,48,0,0,0,63.66,70l14.73,16.2A112,112,0,0,1,128,192Zm6-95.43a8,8,0,0,1,3-15.72,48.16,48.16,0,0,1,38.77,42.64,8,8,0,0,1-7.22,8.71,6.39,6.39,0,0,1-.75,0,8,8,0,0,1-8-7.26A32.09,32.09,0,0,0,134,96.57Zm113.28,34.69c-.42.94-10.55,23.37-33.36,43.8a8,8,0,1,1-10.67-11.92A132.77,132.77,0,0,0,231.05,128a133.15,133.15,0,0,0-23.12-30.77C185.67,75.19,158.78,64,128,64a118.37,118.37,0,0,0-19.36,1.57A8,8,0,1,1,106,49.79,134,134,0,0,1,128,48c34.88,0,66.57,13.26,91.66,38.35,18.83,18.83,27.3,37.62,27.65,38.41A8,8,0,0,1,247.31,131.26Z"/></svg>`;

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

  function shortDisplayPath(fullPath) {
    if (!fullPath) return '';
    // Condense C:\Users\<name>\... to ~\... for crisp scanning while keeping full path in title & confirm
    const m = fullPath.match(/^[A-Za-z]:\\Users\\[^\\]+\\(.+)$/i);
    if (m) return `~\\${m[1]}`;
    return fullPath.replace(/^[A-Za-z]:\\/, '');
  }

  function confLabel(conf) {
    if (conf === 'confirmed') return 'Confirmed';
    if (conf === 'likely') return 'Likely';
    return 'Review';
  }

  const state = {
    activeDrive: 'C:\\',
    drives: [],
    ready: false,
    expanded: new Set(),            // tree view expanded keys
    childrenCache: new Map(),       // key -> children payload
    scanning: false,
    leftoversOnly: false,
    orphanCount: 0,
    orphanTotalBytes: 0,
    orphanTinyCount: 0,
    orphanIgnoredCount: 0,
    // Leftovers view state
    orphanData: null,
    orphanConfidence: 'all',        // 'all' | 'confirmed' | 'likely' | 'review'
    includeTiny: false,
    expandedGroups: new Set(),      // app_key set for multi-folder groups
    expandedPreview: new Set(),     // key set for inline file preview inside leftovers
    selectedPaths: new Map(),       // path -> { size, name, appLabel }
    confirmingKey: null,            // tree view or single leftover confirm key
    confirmingGroupKey: null,       // multi-folder group confirm key
    batchConfirming: false,
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
  const elLeftoversRoot = document.getElementById('leftovers-root');

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

  function renderHeader(data) {
    state.drives = data.drives || [];
    state.activeDrive = data.active_drive || state.activeDrive;
    state.ready = Boolean(data.ready);
    state.orphanCount = data.orphan_count || 0;
    state.orphanTotalBytes = data.orphan_total_bytes || 0;
    state.orphanTinyCount = data.orphan_tiny_count || 0;
    state.orphanIgnoredCount = data.orphan_ignored_count || 0;

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

    if (state.ready || state.leftoversOnly) {
      elLeftovers.hidden = false;
      elLeftovers.setAttribute('aria-pressed', state.leftoversOnly ? 'true' : 'false');
      if (state.orphanCount > 0) {
        elLeftovers.textContent = `Leftovers · ${formatBytes(state.orphanTotalBytes)}`;
      } else if (state.orphanTinyCount > 0) {
        elLeftovers.textContent = `Leftovers · ${state.orphanTinyCount} empty`;
      } else {
        elLeftovers.textContent = 'Leftovers';
      }
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

  async function fetchOrphans() {
    const params = new URLSearchParams({
      include_tiny: state.includeTiny ? '1' : '0',
      confidence: state.orphanConfidence,
    });
    return apiGet(`/api/orphans?${params.toString()}`);
  }

  // =========================================================================
  // Normal Drive Tree View
  // =========================================================================

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

    if (row.leftover_reason && row.leftover_confidence) {
      const tagSpan = document.createElement('span');
      tagSpan.className = 'leftover-tag';
      tagSpan.dataset.confidence = row.leftover_confidence;
      tagSpan.title = row.leftover_reason;
      tagSpan.textContent =
        row.leftover_confidence === 'confirmed' ? 'Confirmed leftover' : 'Likely leftover';
      toggleBtn.appendChild(tagSpan);
    }

    if (row.expandable) {
      toggleBtn.addEventListener('click', () => toggleExpand(row, depth, wrapper, treeRow, toggleBtn));
    }

    const sizeSpan = document.createElement('span');
    sizeSpan.className = 'row-size';
    sizeSpan.dataset.tier = sizeTier(row.size);
    sizeSpan.textContent = formatBytes(row.size);

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
      wrapper.appendChild(buildConfirmRow(row, deleteBtn));
    }

    if (isExpanded) {
      const cached = state.childrenCache.get(row.key);
      if (cached) {
        wrapper.appendChild(buildChildrenGroup(row.key, cached, depth + 1));
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
          group.replaceWith(buildChildrenGroup(parentKey, merged, childDepth));
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
    state.confirmingGroupKey = null;
    state.batchConfirming = false;
    for (const existing of document.querySelectorAll('.confirm-row')) {
      existing.remove();
    }
  }

  function buildConfirmRow(row, focusBackEl) {
    const confirmBar = document.createElement('div');
    confirmBar.className = 'confirm-row';
    confirmBar.setAttribute('role', 'region');
    confirmBar.setAttribute('aria-label', 'Confirm move to Recycle Bin');

    const text = document.createElement('div');
    text.className = 'confirm-text';

    const titleLine = document.createElement('span');
    titleLine.className = 'confirm-title';
    titleLine.textContent = `Move ${row.name} (${formatBytes(row.size)}) to Recycle Bin?`;

    const pathLine = document.createElement('span');
    pathLine.className = 'confirm-path';
    pathLine.textContent = row.path;

    text.appendChild(titleLine);
    text.appendChild(pathLine);

    const actionsWrap = document.createElement('div');
    actionsWrap.className = 'confirm-actions';

    const cancelBtn = document.createElement('button');
    cancelBtn.type = 'button';
    cancelBtn.className = 'confirm-cancel-btn';
    cancelBtn.textContent = 'Cancel';
    cancelBtn.addEventListener('click', () => {
      closeOpenConfirm();
      if (focusBackEl) focusBackEl.focus();
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
        state.selectedPaths.delete(row.path);
        evictCachedPrefix(row.path);
        showNotice(`Moved ${row.name} to Recycle Bin.`);
        await refreshActiveView();
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

  function evictCachedPrefix(pathStr) {
    const lowerPrefix = pathStr.toLowerCase();
    for (const k of Array.from(state.expanded)) {
      if (k.toLowerCase().includes(lowerPrefix)) state.expanded.delete(k);
    }
    for (const k of Array.from(state.expandedPreview)) {
      if (k.toLowerCase().includes(lowerPrefix)) state.expandedPreview.delete(k);
    }
    for (const k of Array.from(state.childrenCache.keys())) {
      if (k.toLowerCase().includes(lowerPrefix)) state.childrenCache.delete(k);
    }
  }

  function toggleDeleteConfirm(row, wrapper, deleteBtn) {
    if (state.confirmingKey === row.key) {
      closeOpenConfirm();
      return;
    }
    closeOpenConfirm();
    state.confirmingKey = row.key;
    const confirmBar = buildConfirmRow(row, deleteBtn);
    const anchor = wrapper.querySelector(':scope > .tree-row, :scope > .leftover-row, :scope > .leftover-subrow');
    if (anchor && anchor.nextSibling) {
      wrapper.insertBefore(confirmBar, anchor.nextSibling);
    } else {
      wrapper.appendChild(confirmBar);
    }
    const cancelBtn = confirmBar.querySelector('.confirm-cancel-btn');
    if (cancelBtn) cancelBtn.focus();
  }

  // =========================================================================
  // Dedicated Leftovers Triage View
  // =========================================================================

  function pruneSelectionsToCurrentData() {
    if (!state.orphanData) return;
    const valid = new Set();
    for (const g of state.orphanData.groups) {
      for (const it of g.items) {
        valid.add(it.path);
      }
    }
    for (const p of Array.from(state.selectedPaths.keys())) {
      if (!valid.has(p)) state.selectedPaths.delete(p);
    }
  }

  async function ignorePaths(paths, label) {
    try {
      await apiPost('/api/orphans/ignore', { paths });
      for (const p of paths) state.selectedPaths.delete(p);
      closeOpenConfirm();
      showNotice(`Kept ${label} — hidden from Leftovers.`);
      await refreshActiveView();
    } catch (err) {
      showNotice(err.message);
    }
  }

  async function deletePathsBatch(paths, summaryLabel) {
    const res = await apiPost('/api/delete-batch', { paths });
    for (const p of paths) {
      state.selectedPaths.delete(p);
      evictCachedPrefix(p);
    }
    closeOpenConfirm();
    if (res.failed && res.failed.length > 0) {
      const firstErr = res.failed[0].error || 'Some items could not be moved.';
      showNotice(`Moved ${res.deleted} of ${paths.length} items. ${firstErr}`);
    } else {
      showNotice(`Moved ${summaryLabel} to Recycle Bin.`);
    }
    await refreshActiveView();
  }

  function buildBatchConfirmBar(paths, totalSize, titleText, focusBackEl) {
    const confirmBar = document.createElement('div');
    confirmBar.className = 'confirm-row';
    confirmBar.setAttribute('role', 'region');
    confirmBar.setAttribute('aria-label', 'Confirm move folders to Recycle Bin');

    const text = document.createElement('div');
    text.className = 'confirm-text';

    const titleLine = document.createElement('span');
    titleLine.className = 'confirm-title';
    titleLine.textContent = `${titleText} (${formatBytes(totalSize)})?`;
    text.appendChild(titleLine);

    const previewPaths = paths.slice(0, 3).map(shortDisplayPath).join('  ·  ');
    const extra = paths.length > 3 ? `  +${paths.length - 3} more` : '';
    const pathLine = document.createElement('span');
    pathLine.className = 'confirm-path';
    pathLine.textContent = `${previewPaths}${extra}`;
    text.appendChild(pathLine);

    const actionsWrap = document.createElement('div');
    actionsWrap.className = 'confirm-actions';

    const cancelBtn = document.createElement('button');
    cancelBtn.type = 'button';
    cancelBtn.className = 'confirm-cancel-btn';
    cancelBtn.textContent = 'Cancel';
    cancelBtn.addEventListener('click', () => {
      closeOpenConfirm();
      if (focusBackEl) focusBackEl.focus();
    });

    const confirmBtn = document.createElement('button');
    confirmBtn.type = 'button';
    confirmBtn.className = 'confirm-delete-btn';
    confirmBtn.textContent = `Move ${paths.length} to Recycle Bin`;
    confirmBtn.addEventListener('click', async () => {
      cancelBtn.disabled = true;
      confirmBtn.disabled = true;
      confirmBtn.textContent = 'Moving…';
      try {
        await deletePathsBatch(
          paths,
          paths.length === 1 ? shortDisplayPath(paths[0]) : `${paths.length} folders`
        );
      } catch (err) {
        cancelBtn.disabled = false;
        confirmBtn.disabled = false;
        confirmBtn.textContent = `Move ${paths.length} to Recycle Bin`;
        showNotice(err.message);
      }
    });

    actionsWrap.appendChild(cancelBtn);
    actionsWrap.appendChild(confirmBtn);
    confirmBar.appendChild(text);
    confirmBar.appendChild(actionsWrap);
    return confirmBar;
  }

  function appendPreviewChildren(hostEl, key, payload) {
    const existing = hostEl.querySelector(':scope > .leftover-preview-children');
    if (existing) existing.remove();
    const wrap = document.createElement('div');
    wrap.className = 'leftover-preview-children';
    wrap.appendChild(buildChildrenGroup(key, payload, 0));
    hostEl.appendChild(wrap);
  }

  function buildLeftoverHeaderLine(appLabel, confidence, folderCount = 0) {
    const line1 = document.createElement('div');
    line1.className = 'leftover-line1';

    const nameEl = document.createElement('span');
    nameEl.className = 'leftover-name';
    nameEl.textContent = appLabel;

    const badge = document.createElement('span');
    badge.className = 'conf-badge';
    badge.dataset.conf = confidence;
    badge.textContent = confLabel(confidence);

    line1.appendChild(nameEl);
    line1.appendChild(badge);

    if (folderCount > 1) {
      const countChip = document.createElement('span');
      countChip.className = 'folder-count-chip';
      countChip.textContent = `${folderCount} folders`;
      line1.appendChild(countChip);
    }

    return line1;
  }

  async function toggleLeftoverPreview(item, hostEl, toggleBtn) {
    const key = item.key;
    if (state.expandedPreview.has(key)) {
      state.expandedPreview.delete(key);
      toggleBtn.setAttribute('aria-expanded', 'false');
      const existing = hostEl.querySelector(':scope > .leftover-preview-children');
      if (existing) existing.remove();
      return;
    }

    state.expandedPreview.add(key);
    toggleBtn.setAttribute('aria-expanded', 'true');

    try {
      let payload = state.childrenCache.get(key);
      if (!payload) {
        payload = await fetchChildrenForKey(key, 0, 200);
        state.childrenCache.set(key, payload);
      }
      if (!state.expandedPreview.has(key)) return;
      appendPreviewChildren(hostEl, key, payload);
    } catch (err) {
      state.expandedPreview.delete(key);
      toggleBtn.setAttribute('aria-expanded', 'false');
      showNotice(err.message);
    }
  }

  function renderLeftoversView() {
    elLeftoversRoot.innerHTML = '';
    const data = state.orphanData;

    // 1. Header & controls
    const header = document.createElement('div');
    header.className = 'leftovers-header';

    const topRow = document.createElement('div');
    topRow.className = 'leftovers-top';

    const headingGroup = document.createElement('div');
    headingGroup.className = 'leftovers-heading-group';

    const backBtn = document.createElement('button');
    backBtn.type = 'button';
    backBtn.className = 'back-btn';
    backBtn.innerHTML = `${SVG_BACK}<span>${state.activeDrive}</span>`;
    backBtn.title = `Back to ${state.activeDrive} folder tree`;
    backBtn.addEventListener('click', () => {
      state.leftoversOnly = false;
      state.selectedPaths.clear();
      closeOpenConfirm();
      refreshActiveView().catch((err) => showNotice(err.message));
    });

    const title = document.createElement('h2');
    title.className = 'leftovers-title';
    title.textContent = 'Leftovers';

    const summary = document.createElement('span');
    summary.className = 'leftovers-summary';
    if (data && data.group_count > 0) {
      const appWord = data.group_count === 1 ? 'app' : 'apps';
      const dirWord = data.folder_count === 1 ? 'folder' : 'folders';
      summary.textContent = `${formatBytes(data.total_bytes)} · ${data.group_count} ${appWord} (${data.folder_count} ${dirWord})`;
    } else {
      summary.textContent = 'No matching leftovers';
    }

    headingGroup.appendChild(backBtn);
    headingGroup.appendChild(title);
    headingGroup.appendChild(summary);

    const auxControls = document.createElement('div');
    auxControls.className = 'leftovers-aux';

    if (data && (data.tiny_count > 0 || state.includeTiny)) {
      const tinyBtn = document.createElement('button');
      tinyBtn.type = 'button';
      tinyBtn.className = 'subtle-toggle';
      tinyBtn.setAttribute('aria-pressed', state.includeTiny ? 'true' : 'false');
      tinyBtn.textContent = state.includeTiny
        ? `Empty folders shown (${data.tiny_count})`
        : `Include empty (${data.tiny_count})`;
      tinyBtn.title = 'Include small or empty (< 64 KB) leftover folders';
      tinyBtn.addEventListener('click', () => {
        state.includeTiny = !state.includeTiny;
        state.selectedPaths.clear();
        closeOpenConfirm();
        refreshActiveView().catch((err) => showNotice(err.message));
      });
      auxControls.appendChild(tinyBtn);
    }

    if (data && data.ignored_count > 0) {
      const resetBtn = document.createElement('button');
      resetBtn.type = 'button';
      resetBtn.className = 'subtle-toggle';
      resetBtn.textContent = `${data.ignored_count} kept · Reset`;
      resetBtn.title = 'Show folders previously marked as kept';
      resetBtn.addEventListener('click', async () => {
        try {
          await apiPost('/api/orphans/clear-ignored', {});
          showNotice('Restored previously kept folders.');
          await refreshActiveView();
        } catch (err) {
          showNotice(err.message);
        }
      });
      auxControls.appendChild(resetBtn);
    }

    topRow.appendChild(headingGroup);
    topRow.appendChild(auxControls);
    header.appendChild(topRow);

    // Confidence tabs + quick select
    if (data && data.counts) {
      const controlsRow = document.createElement('div');
      controlsRow.className = 'leftovers-controls';

      const tabs = document.createElement('div');
      tabs.className = 'confidence-tabs';
      tabs.setAttribute('role', 'tablist');
      tabs.setAttribute('aria-label', 'Filter by confidence');

      const tabDefs = [
        ['all', `All (${data.counts.all})`],
        ['confirmed', `Confirmed (${data.counts.confirmed})`],
        ['likely', `Likely (${data.counts.likely})`],
        ['review', `Review (${data.counts.review})`],
      ];

      for (const [id, label] of tabDefs) {
        const btn = document.createElement('button');
        btn.type = 'button';
        btn.className = 'conf-tab';
        btn.dataset.tab = id;
        btn.setAttribute('role', 'tab');
        btn.setAttribute('aria-selected', state.orphanConfidence === id ? 'true' : 'false');
        btn.textContent = label;
        btn.addEventListener('click', () => {
          if (state.orphanConfidence === id) return;
          state.orphanConfidence = id;
          state.selectedPaths.clear();
          closeOpenConfirm();
          refreshActiveView().catch((err) => showNotice(err.message));
        });
        tabs.appendChild(btn);
      }

      controlsRow.appendChild(tabs);

      const confirmedItems = [];
      for (const g of data.groups) {
        if (g.confidence === 'confirmed') {
          for (const it of g.items) {
            if (it.can_delete) confirmedItems.push({ ...it, appLabel: g.app_label });
          }
        }
      }
      if (confirmedItems.length > 0 && state.selectedPaths.size === 0) {
        const selectConfBtn = document.createElement('button');
        selectConfBtn.type = 'button';
        selectConfBtn.className = 'subtle-toggle';
        selectConfBtn.textContent = `Select confirmed (${confirmedItems.length})`;
        selectConfBtn.addEventListener('click', () => {
          for (const it of confirmedItems) {
            state.selectedPaths.set(it.path, {
              size: it.size,
              name: it.name,
              appLabel: it.appLabel,
            });
          }
          renderLeftoversView();
        });
        controlsRow.appendChild(selectConfBtn);
      }

      header.appendChild(controlsRow);
    }

    elLeftoversRoot.appendChild(header);

    // 2. Contextual batch action bar
    if (state.selectedPaths.size > 0) {
      const selEntries = Array.from(state.selectedPaths.entries());
      const selPaths = selEntries.map(([p]) => p);
      const selBytes = selEntries.reduce((acc, [, v]) => acc + (v.size || 0), 0);

      const batchBar = document.createElement('div');
      batchBar.className = 'batch-bar';

      const info = document.createElement('div');
      info.className = 'batch-info';
      const dirWord = selPaths.length === 1 ? 'folder' : 'folders';
      const infoText = document.createElement('span');
      infoText.textContent = `${selPaths.length} ${dirWord} selected · ${formatBytes(selBytes)}`;
      const clearBtn = document.createElement('button');
      clearBtn.type = 'button';
      clearBtn.className = 'subtle-toggle';
      clearBtn.textContent = 'Clear';
      clearBtn.addEventListener('click', () => {
        state.selectedPaths.clear();
        state.batchConfirming = false;
        renderLeftoversView();
      });
      info.appendChild(infoText);
      info.appendChild(clearBtn);

      const batchActs = document.createElement('div');
      batchActs.className = 'batch-actions';

      const keepSelBtn = document.createElement('button');
      keepSelBtn.type = 'button';
      keepSelBtn.className = 'confirm-cancel-btn';
      keepSelBtn.textContent = 'Keep selected';
      keepSelBtn.addEventListener('click', () => {
        ignorePaths(selPaths, `${selPaths.length} ${dirWord}`);
      });

      const delSelBtn = document.createElement('button');
      delSelBtn.type = 'button';
      delSelBtn.className = 'confirm-delete-btn';
      delSelBtn.textContent = 'Move to Recycle Bin';
      delSelBtn.addEventListener('click', () => {
        if (state.batchConfirming) {
          state.batchConfirming = false;
          renderLeftoversView();
          return;
        }
        closeOpenConfirm();
        state.batchConfirming = true;
        renderLeftoversView();
      });

      batchActs.appendChild(keepSelBtn);
      batchActs.appendChild(delSelBtn);
      batchBar.appendChild(info);
      batchBar.appendChild(batchActs);
      elLeftoversRoot.appendChild(batchBar);

      if (state.batchConfirming) {
        elLeftoversRoot.appendChild(
          buildBatchConfirmBar(
            selPaths,
            selBytes,
            `Move ${selPaths.length} selected ${dirWord} to Recycle Bin`,
            delSelBtn
          )
        );
      }
    }

    // 3. Empty state
    if (!data || data.groups.length === 0) {
      const empty = document.createElement('div');
      empty.className = 'empty-state';
      empty.textContent =
        state.orphanConfidence !== 'all'
          ? `No ${state.orphanConfidence} leftovers found on ${state.activeDrive}.`
          : `No leftover folders from uninstalled apps detected on ${state.activeDrive}.`;

      const emptyActions = document.createElement('div');
      emptyActions.className = 'empty-state-actions';
      if (state.orphanConfidence !== 'all') {
        const showAll = document.createElement('button');
        showAll.type = 'button';
        showAll.className = 'confirm-cancel-btn';
        showAll.textContent = 'Show all tiers';
        showAll.addEventListener('click', () => {
          state.orphanConfidence = 'all';
          refreshActiveView().catch((err) => showNotice(err.message));
        });
        emptyActions.appendChild(showAll);
      }
      const backHome = document.createElement('button');
      backHome.type = 'button';
      backHome.className = 'confirm-cancel-btn';
      backHome.textContent = `Return to ${state.activeDrive}`;
      backHome.addEventListener('click', () => {
        state.leftoversOnly = false;
        refreshActiveView().catch((err) => showNotice(err.message));
      });
      emptyActions.appendChild(backHome);
      empty.appendChild(emptyActions);
      elLeftoversRoot.appendChild(empty);
      return;
    }

    // 4. Grouped list
    const list = document.createElement('div');
    list.className = 'leftovers-list';

    for (const group of data.groups) {
      if (group.items.length === 1) {
        list.appendChild(createSingleLeftoverRow(group, group.items[0]));
      } else {
        list.appendChild(createMultiLeftoverGroup(group));
      }
    }

    elLeftoversRoot.appendChild(list);
  }

  function createSingleLeftoverRow(group, item) {
    const wrap = document.createElement('div');
    wrap.className = 'leftover-group';
    wrap.dataset.key = item.key;

    const row = document.createElement('div');
    row.className = 'leftover-row';

    // Checkbox
    const cb = document.createElement('input');
    cb.type = 'checkbox';
    cb.className = 'leftover-check';
    cb.checked = state.selectedPaths.has(item.path);
    cb.disabled = !item.can_delete;
    cb.setAttribute('aria-label', `Select ${group.app_label}`);
    cb.addEventListener('change', () => {
      if (cb.checked) {
        state.selectedPaths.set(item.path, {
          size: item.size,
          name: item.name,
          appLabel: group.app_label,
        });
      } else {
        state.selectedPaths.delete(item.path);
      }
      renderLeftoversView();
    });

    // Main 2-line clickable preview toggle
    const isPreviewOpen = item.expandable && state.expandedPreview.has(item.key);
    const mainBtn = document.createElement('button');
    mainBtn.type = 'button';
    mainBtn.className = 'leftover-main';
    mainBtn.dataset.expandable = item.expandable ? 'true' : 'false';
    if (item.expandable) {
      mainBtn.setAttribute('aria-expanded', isPreviewOpen ? 'true' : 'false');
    }
    mainBtn.title = `${item.path}\n${item.reason}`;

    const caret = document.createElement('span');
    caret.className = item.expandable ? 'caret caret-chevron' : 'caret caret-dot';
    caret.innerHTML = item.expandable ? SVG_CARET : SVG_DOT;

    const textCol = document.createElement('div');
    textCol.className = 'leftover-text';

    const line1 = buildLeftoverHeaderLine(group.app_label, group.confidence);

    const line2 = document.createElement('div');
    line2.className = 'leftover-line2';

    const pathSpan = document.createElement('span');
    pathSpan.className = 'leftover-path-inline';
    pathSpan.textContent = shortDisplayPath(item.path);

    const sep = document.createElement('span');
    sep.className = 'leftover-sep';
    sep.textContent = '·';

    const evSpan = document.createElement('span');
    evSpan.className = 'leftover-evidence';
    evSpan.textContent = item.reason;

    line2.appendChild(pathSpan);
    line2.appendChild(sep);
    line2.appendChild(evSpan);

    textCol.appendChild(line1);
    textCol.appendChild(line2);
    mainBtn.appendChild(caret);
    mainBtn.appendChild(textCol);

    if (item.expandable) {
      mainBtn.addEventListener('click', () => toggleLeftoverPreview(item, wrap, mainBtn));
    }

    const sizeSpan = document.createElement('span');
    sizeSpan.className = 'row-size';
    sizeSpan.dataset.tier = sizeTier(item.size);
    sizeSpan.textContent = formatBytes(item.size);

    const actions = document.createElement('div');
    actions.className = 'row-actions';

    const explorerBtn = document.createElement('button');
    explorerBtn.type = 'button';
    explorerBtn.className = 'icon-btn';
    explorerBtn.setAttribute('aria-label', `Open ${item.name} in File Explorer`);
    explorerBtn.title = 'Open in File Explorer';
    explorerBtn.innerHTML = SVG_FOLDER;
    explorerBtn.addEventListener('click', async (e) => {
      e.stopPropagation();
      try {
        await apiPost('/api/open-explorer', { path: item.path, select_file: false });
      } catch (err) {
        showNotice(err.message);
      }
    });

    const keepBtn = document.createElement('button');
    keepBtn.type = 'button';
    keepBtn.className = 'icon-btn';
    keepBtn.setAttribute('aria-label', `Keep ${group.app_label} (not a leftover)`);
    keepBtn.title = 'Keep (not a leftover)';
    keepBtn.innerHTML = SVG_KEEP;
    keepBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      ignorePaths([item.path], group.app_label);
    });

    const delBtn = document.createElement('button');
    delBtn.type = 'button';
    delBtn.className = 'icon-btn icon-btn-danger';
    delBtn.innerHTML = SVG_TRASH;
    if (!item.can_delete) {
      delBtn.disabled = true;
      delBtn.title = 'Protected item';
    } else {
      delBtn.setAttribute('aria-label', `Move ${item.name} to Recycle Bin`);
      delBtn.title = 'Move to Recycle Bin';
      delBtn.addEventListener('click', (e) => {
        e.stopPropagation();
        toggleDeleteConfirm(
          { key: item.key, name: group.app_label, path: item.path, size: item.size },
          wrap,
          delBtn
        );
      });
    }

    actions.appendChild(explorerBtn);
    actions.appendChild(keepBtn);
    actions.appendChild(delBtn);

    row.appendChild(cb);
    row.appendChild(mainBtn);
    row.appendChild(sizeSpan);
    row.appendChild(actions);
    wrap.appendChild(row);

    if (item.can_delete && state.confirmingKey === item.key) {
      wrap.appendChild(
        buildConfirmRow(
          { key: item.key, name: group.app_label, path: item.path, size: item.size },
          delBtn
        )
      );
    }

    if (isPreviewOpen) {
      const cached = state.childrenCache.get(item.key);
      if (cached) appendPreviewChildren(wrap, item.key, cached);
    }

    return wrap;
  }

  function createMultiLeftoverGroup(group) {
    const wrap = document.createElement('div');
    wrap.className = 'leftover-group';
    wrap.dataset.groupKey = group.app_key;

    const deletableItems = group.items.filter((i) => i.can_delete);
    const selectedCount = deletableItems.filter((i) => state.selectedPaths.has(i.path)).length;
    const allSelected = deletableItems.length > 0 && selectedCount === deletableItems.length;
    const someSelected = selectedCount > 0 && selectedCount < deletableItems.length;
    const isExpanded = state.expandedGroups.has(group.app_key);

    const row = document.createElement('div');
    row.className = 'leftover-row';

    const cb = document.createElement('input');
    cb.type = 'checkbox';
    cb.className = 'leftover-check';
    cb.checked = allSelected;
    cb.indeterminate = someSelected;
    cb.disabled = deletableItems.length === 0;
    cb.setAttribute('aria-label', `Select all ${group.items.length} folders for ${group.app_label}`);
    cb.addEventListener('change', () => {
      if (cb.checked) {
        for (const it of deletableItems) {
          state.selectedPaths.set(it.path, {
            size: it.size,
            name: it.name,
            appLabel: group.app_label,
          });
        }
      } else {
        for (const it of deletableItems) {
          state.selectedPaths.delete(it.path);
        }
      }
      renderLeftoversView();
    });

    const mainBtn = document.createElement('button');
    mainBtn.type = 'button';
    mainBtn.className = 'leftover-main';
    mainBtn.dataset.expandable = 'true';
    mainBtn.setAttribute('aria-expanded', isExpanded ? 'true' : 'false');
    mainBtn.title = `${group.app_label} (${group.items.length} folders)\n${group.summary_reason}`;
    mainBtn.addEventListener('click', () => {
      if (state.expandedGroups.has(group.app_key)) {
        state.expandedGroups.delete(group.app_key);
      } else {
        state.expandedGroups.add(group.app_key);
      }
      renderLeftoversView();
    });

    const caret = document.createElement('span');
    caret.className = 'caret caret-chevron';
    caret.innerHTML = SVG_CARET;

    const textCol = document.createElement('div');
    textCol.className = 'leftover-text';

    const line1 = buildLeftoverHeaderLine(
      group.app_label,
      group.confidence,
      group.items.length
    );

    const line2 = document.createElement('div');
    line2.className = 'leftover-line2';

    const evSpan = document.createElement('span');
    evSpan.className = 'leftover-evidence';
    evSpan.textContent = group.summary_reason;

    line2.appendChild(evSpan);

    textCol.appendChild(line1);
    textCol.appendChild(line2);
    mainBtn.appendChild(caret);
    mainBtn.appendChild(textCol);

    const sizeSpan = document.createElement('span');
    sizeSpan.className = 'row-size';
    sizeSpan.dataset.tier = sizeTier(group.total_size);
    sizeSpan.textContent = formatBytes(group.total_size);

    const actions = document.createElement('div');
    actions.className = 'row-actions';

    // Open largest fragment in Explorer
    const explorerBtn = document.createElement('button');
    explorerBtn.type = 'button';
    explorerBtn.className = 'icon-btn';
    explorerBtn.setAttribute('aria-label', `Open main ${group.app_label} folder in File Explorer`);
    explorerBtn.title = `Open ${shortDisplayPath(group.items[0].path)} in File Explorer`;
    explorerBtn.innerHTML = SVG_FOLDER;
    explorerBtn.addEventListener('click', async (e) => {
      e.stopPropagation();
      try {
        await apiPost('/api/open-explorer', { path: group.items[0].path, select_file: false });
      } catch (err) {
        showNotice(err.message);
      }
    });

    const keepBtn = document.createElement('button');
    keepBtn.type = 'button';
    keepBtn.className = 'icon-btn';
    keepBtn.setAttribute('aria-label', `Keep ${group.app_label} (not a leftover)`);
    keepBtn.title = `Keep all ${group.items.length} folders (not a leftover)`;
    keepBtn.innerHTML = SVG_KEEP;
    keepBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      ignorePaths(
        group.items.map((i) => i.path),
        group.app_label
      );
    });

    const delBtn = document.createElement('button');
    delBtn.type = 'button';
    delBtn.className = 'icon-btn icon-btn-danger';
    delBtn.innerHTML = SVG_TRASH;
    if (deletableItems.length === 0) {
      delBtn.disabled = true;
      delBtn.title = 'Protected items';
    } else {
      delBtn.setAttribute(
        'aria-label',
        `Move all ${deletableItems.length} ${group.app_label} folders to Recycle Bin`
      );
      delBtn.title = `Move all ${deletableItems.length} folders to Recycle Bin`;
      delBtn.addEventListener('click', (e) => {
        e.stopPropagation();
        if (state.confirmingGroupKey === group.app_key) {
          closeOpenConfirm();
          return;
        }
        closeOpenConfirm();
        state.confirmingGroupKey = group.app_key;
        renderLeftoversView();
      });
    }

    actions.appendChild(explorerBtn);
    actions.appendChild(keepBtn);
    actions.appendChild(delBtn);

    row.appendChild(cb);
    row.appendChild(mainBtn);
    row.appendChild(sizeSpan);
    row.appendChild(actions);
    wrap.appendChild(row);

    if (state.confirmingGroupKey === group.app_key) {
      const delPaths = deletableItems.map((i) => i.path);
      const delSize = deletableItems.reduce((a, i) => a + i.size, 0);
      wrap.appendChild(
        buildBatchConfirmBar(
          delPaths,
          delSize,
          `Move all ${delPaths.length} ${group.app_label} folders to Recycle Bin`,
          delBtn
        )
      );
    }

    if (isExpanded) {
      const subList = document.createElement('div');
      subList.className = 'leftover-sublist';
      for (const item of group.items) {
        subList.appendChild(createLeftoverSubRow(group, item));
      }
      wrap.appendChild(subList);
    }

    return wrap;
  }

  function createLeftoverSubRow(group, item) {
    const subWrap = document.createElement('div');
    subWrap.className = 'tree-node';
    subWrap.dataset.key = item.key;

    const subRow = document.createElement('div');
    subRow.className = 'leftover-subrow';

    const cb = document.createElement('input');
    cb.type = 'checkbox';
    cb.className = 'leftover-check';
    cb.checked = state.selectedPaths.has(item.path);
    cb.disabled = !item.can_delete;
    cb.setAttribute('aria-label', `Select ${item.path}`);
    cb.addEventListener('change', () => {
      if (cb.checked) {
        state.selectedPaths.set(item.path, {
          size: item.size,
          name: item.name,
          appLabel: group.app_label,
        });
      } else {
        state.selectedPaths.delete(item.path);
      }
      renderLeftoversView();
    });

    const isPreviewOpen = item.expandable && state.expandedPreview.has(item.key);
    const mainBtn = document.createElement('button');
    mainBtn.type = 'button';
    mainBtn.className = 'leftover-main';
    mainBtn.dataset.expandable = item.expandable ? 'true' : 'false';
    if (item.expandable) {
      mainBtn.setAttribute('aria-expanded', isPreviewOpen ? 'true' : 'false');
    }
    mainBtn.title = `${item.path}\n${item.reason}`;

    const caret = document.createElement('span');
    caret.className = item.expandable ? 'caret caret-chevron' : 'caret caret-dot';
    caret.innerHTML = item.expandable ? SVG_CARET : SVG_DOT;

    const pathEl = document.createElement('span');
    pathEl.className = 'leftover-sub-path';
    pathEl.textContent = shortDisplayPath(item.path);

    mainBtn.appendChild(caret);
    mainBtn.appendChild(pathEl);

    if (item.expandable) {
      mainBtn.addEventListener('click', () => toggleLeftoverPreview(item, subWrap, mainBtn));
    }

    const sizeSpan = document.createElement('span');
    sizeSpan.className = 'row-size';
    sizeSpan.dataset.tier = sizeTier(item.size);
    sizeSpan.textContent = formatBytes(item.size);

    const actions = document.createElement('div');
    actions.className = 'row-actions';

    const explorerBtn = document.createElement('button');
    explorerBtn.type = 'button';
    explorerBtn.className = 'icon-btn';
    explorerBtn.setAttribute('aria-label', `Open ${item.path} in File Explorer`);
    explorerBtn.title = 'Open in File Explorer';
    explorerBtn.innerHTML = SVG_FOLDER;
    explorerBtn.addEventListener('click', async (e) => {
      e.stopPropagation();
      try {
        await apiPost('/api/open-explorer', { path: item.path, select_file: false });
      } catch (err) {
        showNotice(err.message);
      }
    });

    const keepBtn = document.createElement('button');
    keepBtn.type = 'button';
    keepBtn.className = 'icon-btn';
    keepBtn.setAttribute('aria-label', `Keep ${item.path}`);
    keepBtn.title = 'Keep (not a leftover)';
    keepBtn.innerHTML = SVG_KEEP;
    keepBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      ignorePaths([item.path], shortDisplayPath(item.path));
    });

    const delBtn = document.createElement('button');
    delBtn.type = 'button';
    delBtn.className = 'icon-btn icon-btn-danger';
    delBtn.innerHTML = SVG_TRASH;
    if (!item.can_delete) {
      delBtn.disabled = true;
      delBtn.title = 'Protected item';
    } else {
      delBtn.setAttribute('aria-label', `Move ${item.path} to Recycle Bin`);
      delBtn.title = 'Move to Recycle Bin';
      delBtn.addEventListener('click', (e) => {
        e.stopPropagation();
        toggleDeleteConfirm(
          { key: item.key, name: item.name, path: item.path, size: item.size },
          subWrap,
          delBtn
        );
      });
    }

    actions.appendChild(explorerBtn);
    actions.appendChild(keepBtn);
    actions.appendChild(delBtn);

    subRow.appendChild(cb);
    subRow.appendChild(mainBtn);
    subRow.appendChild(sizeSpan);
    subRow.appendChild(actions);
    subWrap.appendChild(subRow);

    if (item.can_delete && state.confirmingKey === item.key) {
      subWrap.appendChild(
        buildConfirmRow(
          { key: item.key, name: item.name, path: item.path, size: item.size },
          delBtn
        )
      );
    }

    if (isPreviewOpen) {
      const cached = state.childrenCache.get(item.key);
      if (cached) appendPreviewChildren(subWrap, item.key, cached);
    }

    return subWrap;
  }

  // =========================================================================
  // View Coordination & Refresh
  // =========================================================================

  async function refreshActiveView() {
    const data = await apiGet('/api/state');
    renderHeader(data);

    if (data.scanning) {
      state.scanning = true;
      const scannedStr = data.scanned_bytes > 0 ? ` · ${formatBytes(data.scanned_bytes)}` : '';
      setStatus(`Scanning${scannedStr}`, true);
    } else {
      state.scanning = false;
      setStatus('', false);
    }

    if (!data.ready) {
      elTreeRoot.hidden = false;
      elLeftoversRoot.hidden = true;
      return;
    }

    if (state.leftoversOnly) {
      elTreeRoot.hidden = true;
      elLeftoversRoot.hidden = false;
      state.orphanData = await fetchOrphans();
      pruneSelectionsToCurrentData();
      renderLeftoversView();
      return;
    }

    elLeftoversRoot.hidden = true;
    elTreeRoot.hidden = false;

    const expandedKeys = Array.from(state.expanded);
    const rootKey = `dir:${data.active_drive}`;
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
        refreshActiveView();
      });
      elTreeRoot.appendChild(moreBtn);
    }
  }

  async function switchDrive(driveRoot) {
    state.expanded.clear();
    state.expandedGroups.clear();
    state.expandedPreview.clear();
    state.selectedPaths.clear();
    state.childrenCache.clear();
    closeOpenConfirm();
    state.leftoversOnly = false;
    setStatus('Scanning', true);
    try {
      await apiPost('/api/rescan', { drive: driveRoot, force: false });
      await refreshActiveView();
    } catch (err) {
      showNotice(err.message);
    }
  }

  elLeftovers.addEventListener('click', () => {
    state.leftoversOnly = !state.leftoversOnly;
    state.selectedPaths.clear();
    closeOpenConfirm();
    elLeftovers.setAttribute('aria-pressed', state.leftoversOnly ? 'true' : 'false');
    refreshActiveView().catch((err) => showNotice(err.message));
  });

  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && (state.confirmingKey || state.confirmingGroupKey || state.batchConfirming)) {
      e.preventDefault();
      const key = state.confirmingKey;
      closeOpenConfirm();
      if (state.leftoversOnly) {
        renderLeftoversView();
      } else if (key) {
        for (const nodeEl of elTreeRoot.querySelectorAll('.tree-node')) {
          if (nodeEl.dataset.key === key) {
            const delBtn = nodeEl.querySelector(':scope > .tree-row .icon-btn-danger');
            if (delBtn && !delBtn.disabled) delBtn.focus();
            break;
          }
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
    es.addEventListener('updated', (ev) => {
      let reason = '';
      try {
        reason = JSON.parse(ev.data).reason || '';
      } catch {}
      // Never nuke the DOM mid-triage on noisy background filesystem watcher ticks!
      if (
        reason === 'watcher' &&
        (state.leftoversOnly || state.confirmingKey || state.confirmingGroupKey || state.batchConfirming)
      ) {
        return;
      }
      refreshActiveView().catch(() => {});
    });
    es.onerror = () => {
      es.close();
      setTimeout(connectEvents, 1500);
    };
  }

  refreshActiveView().catch((err) => showNotice(err.message));
  connectEvents();
})();
