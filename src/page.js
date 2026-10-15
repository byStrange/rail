// Injected into every page (isolated world "rail"): vim-style keys that must
// work inside the page, because only the page knows what is being edited.
(() => {
  if (window.__rail) return;
  window.__rail = true;
  const OFF = /*OFF*/[];
  const send = (m) => { try { window.webkit.messageHandlers.rail.postMessage(m); } catch (e) {} };

  // ---- typing in a field marks the tab as "has unsaved input" (never suspended) ----
  let dirtySent = false;
  const dirty = (e) => { if (e.isTrusted && !dirtySent) { dirtySent = true; send('dirty'); } };
  addEventListener('input', dirty, true);
  addEventListener('change', dirty, true);

  const kinds = { TEXTAREA: 1, SELECT: 1 };
  const textTypes = ['button', 'checkbox', 'radio', 'submit', 'reset', 'image', 'file', 'range', 'color'];
  function target(e) {
    const p = e.composedPath ? e.composedPath() : [];
    return p.length ? p[0] : e.target;
  }
  function editing(t) {
    if (!t || t.nodeType !== 1) return false;
    if (t.isContentEditable || kinds[t.tagName]) return true;
    if (t.tagName === 'INPUT') return !textTypes.includes((t.type || '').toLowerCase());
    return false;
  }
  function clickable(t) {
    return !!(t && t.nodeType === 1 && t.closest && t.closest('button,input,summary,select,textarea,[role=button],[role=checkbox],[role=menuitem],[role=tab]'));
  }

  // ---- smooth scrolling ----
  function scroller() {
    let el = document.activeElement;
    if (!el || el === document.body || el === document.documentElement) {
      el = document.elementFromPoint(innerWidth / 2, innerHeight / 2);
    }
    for (let e = el; e && e !== document.body && e !== document.documentElement; e = e.parentElement) {
      const s = getComputedStyle(e);
      if (/(auto|scroll)/.test(s.overflowY) && e.scrollHeight > e.clientHeight + 1) return e;
    }
    return document.scrollingElement || document.documentElement;
  }
  const isRoot = (el) => el === document.scrollingElement || el === document.documentElement;
  const cur = (el) => (isRoot(el) ? window.scrollY : el.scrollTop);
  const put = (el, v) => { if (isRoot(el)) window.scrollTo(window.scrollX, v); else el.scrollTop = v; };
  const maxOf = (el) => (isRoot(el) ? document.documentElement.scrollHeight - innerHeight : el.scrollHeight - el.clientHeight);
  const viewH = (el) => (isRoot(el) ? innerHeight : el.clientHeight);
  let tgt = null, el0 = null, raf = 0;
  function step() {
    const c = cur(el0), d = tgt - c;
    if (Math.abs(d) < 1) { put(el0, tgt); raf = 0; tgt = null; return; }
    put(el0, c + d * 0.28 + Math.sign(d) * 0.5);
    raf = requestAnimationFrame(step);
  }
  function scrollBy(dy) {
    const el = scroller();
    if (el !== el0 || tgt === null) { el0 = el; tgt = cur(el); }
    tgt = Math.max(0, Math.min(maxOf(el), tgt + dy));
    if (!raf) raf = requestAnimationFrame(step);
  }
  function scrollTo(frac) {
    const el = scroller();
    el0 = el;
    tgt = frac === 0 ? 0 : maxOf(el);
    if (!raf) raf = requestAnimationFrame(step);
  }
  addEventListener('wheel', () => { tgt = null; }, { passive: true, capture: true });

  // A Latin letter means what it says (Dvorak, Colemak...); on any other layout
  // fall back to the physical key, so the shortcuts still work.
  function keyName(e) {
    const k = e.key;
    if (k && k.length === 1) {
      if (/[a-z]/i.test(k)) return 'Key' + k.toUpperCase();
      if (k === '/') return 'Slash';
      if (k === ' ') return 'Space';
    }
    return e.code;
  }

  // ---- key handling ----
  let seq = '', seqT = 0;
  const SMALL = 90;
  function onKey(e) {
    if (!e.isTrusted || e.ctrlKey || e.altKey || e.metaKey || e.isComposing) return;
    if (window.__railPip) return pipKey(e);
    if (OFF.includes(location.hostname.replace(/^www\./, ''))) return;
    const t = target(e);
    if (editing(t)) { seq = ''; return; }
    const now = performance.now();
    if (now - seqT > 900) seq = '';
    const c = keyName(e), sh = e.shiftKey;
    const eat = () => { e.preventDefault(); e.stopPropagation(); e.stopImmediatePropagation(); };

    // <leader> f w  ->  fuzzy tab search
    if (seq === 'space' && c === 'KeyF' && !sh) { seq = 'f'; seqT = now; return eat(); }
    if (seq === 'f' && c === 'KeyW' && !sh) { seq = ''; send('tabs'); return eat(); }
    if (seq === 'g' && c === 'KeyG' && !sh) { seq = ''; scrollTo(0); return eat(); }
    seq = '';

    if (c === 'Space' && !sh) {
      // Space already means play / pause on a page that is playing something.
      if (clickable(t) || [...document.querySelectorAll('video,audio')].some((m) => !m.paused)) return;
      scrollBy(SMALL);
      seq = 'space'; seqT = now;
      return eat();
    }
    if (sh && c !== 'KeyG' && c !== 'KeyN') return;
    switch (c) {
      case 'KeyJ': scrollBy(SMALL); return eat();
      case 'KeyK': scrollBy(-SMALL); return eat();
      case 'KeyD': scrollBy(viewH(scroller()) / 2); return eat();
      case 'KeyU': scrollBy(-viewH(scroller()) / 2); return eat();
      case 'KeyG':
        if (sh) { scrollTo(1); return eat(); }
        seq = 'g'; seqT = now; return eat();
      case 'Slash': send('find'); return eat();
      case 'KeyN':
        if (window.__railFind) { send(sh ? 'prev' : 'next'); return eat(); }
    }
  }
  addEventListener('keydown', onKey, true);

  // ---- picture-in-picture: the tab is popped out into a small window and the
  //      biggest video is stretched over the whole page ----
  let pip = null, lastV = null, wasPlaying = false;
  function pipClick(e) {
    if (!window.__railPip || !pip) return;
    e.preventDefault(); e.stopPropagation();
    const v = pip.v;
    if (v.paused) v.play(); else v.pause();
  }
  function pipKey(e) {
    const v = pip && pip.v;
    if (!v) return;
    let used = true;
    if (e.code === 'Space') { if (v.paused) v.play(); else v.pause(); }
    else if (e.code === 'ArrowLeft') v.currentTime -= 5;
    else if (e.code === 'ArrowRight') v.currentTime += 5;
    else if (e.code === 'KeyM') v.muted = !v.muted;
    else used = false;
    if (used) { e.preventDefault(); e.stopPropagation(); e.stopImmediatePropagation(); }
  }
  window.__railPipOn = () => {
    if (pip) return JSON.stringify([pip.v.videoWidth, pip.v.videoHeight]);
    const vs = [...document.querySelectorAll('video')].filter((v) => v.videoWidth > 0);
    if (!vs.length) return 'null';
    const rank = (v) => { const r = v.getBoundingClientRect(); return (v.paused ? 0 : 1e9) + r.width * r.height; };
    vs.sort((a, b) => rank(b) - rank(a));
    const v = vs[0];
    // Nothing about the video's box type changes (a display / position change rebuilds its
    // renderer and the picture stops): hide every other element and size the chain to the page.
    const saved = [];
    const set = (el, props) => {
      saved.push([el, el.getAttribute('style')]);
      for (const [k, val] of props) el.style.setProperty(k, val, 'important');
    };
    const FIT = [['width', '100vw'], ['height', '100vh'], ['min-width', '0'], ['min-height', '0'],
      ['max-width', 'none'], ['max-height', 'none'], ['margin', '0'], ['padding', '0'], ['border', '0'],
      ['transform', 'none'], ['filter', 'none'], ['overflow', 'visible'], ['flex', 'none'], ['aspect-ratio', 'auto']];
    for (let e = v; e && e !== document.body && e !== document.documentElement; e = e.parentElement) {
      for (const sib of e.parentElement.children) if (sib !== e) set(sib, [['display', 'none']]);
      set(e, FIT);
    }
    set(v, [['object-fit', 'contain'], ['background', '#000']]);
    set(document.body, [['margin', '0'], ['padding', '0'], ['overflow', 'hidden'], ['background', '#000']]);
    set(document.documentElement, [['overflow', 'hidden'], ['background', '#000']]);
    const scroll = [window.scrollX, window.scrollY];
    window.scrollTo(0, 0);
    lastV = v;
    wasPlaying = !v.paused;
    pip = { v, saved, scroll };
    window.__railPip = true;
    addEventListener('click', pipClick, true);
    return JSON.stringify([v.videoWidth, v.videoHeight]);
  };
  // Moving the page between windows makes WebKit pause it; carry on if it was playing.
  window.__railKick = () => { if (lastV && wasPlaying && lastV.paused) lastV.play().catch(() => {}); };
  window.__railPipOff = () => {
    if (!pip) return;
    removeEventListener('click', pipClick, true);
    for (const [e, s] of pip.saved.reverse()) { if (s === null) e.removeAttribute('style'); else e.setAttribute('style', s); }
    window.scrollTo(pip.scroll[0], pip.scroll[1]);
    pip = null;
    window.__railPip = false;
  };
})();
