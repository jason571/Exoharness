// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

/** Variable-height history window. Live rows remain mounted; no model data is changed. */
export class VirtualTurnList<T> {
  private items: T[] = [];
  private nodes = new Map<string, HTMLElement>();
  private heights = new Map<string, number>();
  private live = new Set<string>();
  private frame = 0;
  private destroyed = false;
  private observer: ResizeObserver;
  private previousScrollBehavior: string;
  private following = false;
  private automaticTop: number | null = null;
  readonly root = document.createElement('div');
  private schedule = (): void => {
    if (!this.frame && !this.destroyed) this.frame = requestAnimationFrame(() => {
      this.frame = 0;
      this.paint();
    });
  };
  private onScroll = (): void => {
    if (this.automaticTop === null || Math.abs(this.scroll.scrollTop - this.automaticTop) > 1) {
      this.following = this.scroll.scrollHeight - this.scroll.scrollTop - this.scroll.clientHeight <= 2;
    }
    this.automaticTop = null;
    this.schedule();
  };
  private move(top: number): void {
    this.scroll.scrollTop = top;
    this.automaticTop = this.scroll.scrollTop;
  }

  constructor(private scroll: HTMLElement, private key: (item: T) => string,
              private render: (item: T) => HTMLElement, private active: (item: T) => boolean) {
    this.root.className = 'chat-virtual-history';
    // Programmatic anchor corrections must be atomic, not CSS smooth scrolls.
    this.previousScrollBehavior = scroll.style.scrollBehavior;
    scroll.style.scrollBehavior = 'auto';
    this.observer = new ResizeObserver(entries => {
      let changed = false;
      for (const entry of entries) {
        const node = entry.target as HTMLElement;
        const id = node.dataset.virtualKey!;
        const height = Math.max(1, node.getBoundingClientRect().height);
        const previous = this.heights.get(id) ?? 420;
        if (Math.abs(height - previous) < .5) continue;
        changed = true;
        this.heights.set(id, height);
      }
      if (changed) this.schedule();
    });
    scroll.addEventListener('scroll', this.onScroll, {passive: true});
    window.addEventListener('resize', this.schedule);
  }

  set(items: T[], prepend = false): void {
    const previous = new Set(this.items.map(this.key));
    const added = prepend ? items.filter(item => !previous.has(this.key(item))) : [];
    this.items = items;
    for (const item of items) if (this.active(item)) this.live.add(this.key(item));
    const correction = added.reduce((sum, item) => sum + (this.heights.get(this.key(item)) ?? 420), 0);
    this.paint(correction);
  }

  reveal(id: string): void {
    this.following = false;
    let offset = 0;
    for (const item of this.items) {
      if (this.key(item) === id) {
        this.move(this.scroll.scrollTop + this.root.getBoundingClientRect().top - this.scroll.getBoundingClientRect().top + offset);
        this.paint();
        this.nodes.get(id)?.scrollIntoView({block: 'start'});
        return;
      }
      offset += this.heights.get(this.key(item)) ?? 420;
    }
  }

  scrollToLatest(): void {
    this.following = true;
    this.move(this.scroll.scrollHeight);
    this.paint();
  }

  recent(limit: number): { item: T; node?: HTMLElement }[] {
    return this.items.slice(-limit).map(item => ({item, node: this.nodes.get(this.key(item))}));
  }

  mounted(id: string): HTMLElement | undefined { return this.nodes.get(id); }

  private paint(correction = 0): void {
    if (this.destroyed || !this.root.isConnected) return;
    const rootTop = this.root.getBoundingClientRect().top;
    const view = this.scroll.getBoundingClientRect();
    const oldScroll = this.scroll.scrollTop;
    const anchor = Array.from(this.root.children).find(node => {
      const rect = node.getBoundingClientRect();
      return node.classList.contains('chat-virtual-turn') && rect.bottom >= view.top && rect.top <= view.bottom;
    });
    const anchorTop = anchor?.getBoundingClientRect().top;
    const anchorKey = (anchor as HTMLElement | undefined)?.dataset.virtualKey;
    const from = view.top - rootTop - 1400 + correction, to = view.bottom - rootTop + 1400 + correction;
    const desired: HTMLElement[] = [];
    const wanted = new Set<string>();
    let offset = 0, skipped = 0;
    const spacer = (): void => {
      if (!skipped) return;
      const node = document.createElement('div');
      node.className = 'chat-virtual-spacer';
      node.setAttribute('aria-hidden', 'true');
      node.style.height = `${skipped}px`;
      desired.push(node);
      skipped = 0;
    };
    for (const item of this.items) {
      const id = this.key(item), height = this.heights.get(id) ?? 420;
      const existing = this.nodes.get(id);
      // A live row is never reconstructed from its stale initial snapshot.
      // Retain it for this view after completion, too, until a session reload.
      const pinned = this.live.has(id);
      const focused = existing?.contains(document.activeElement);
      const media = existing && Array.from(existing.querySelectorAll<HTMLMediaElement>('video,audio')).some(media => !media.paused);
      // Height estimates may differ greatly on narrow screens. Never discard
      // the visible anchor before measuring the replacement window.
      if (offset + height >= from && offset <= to || id === anchorKey || pinned || focused || media) {
        spacer();
        let node = existing;
        if (!node) {
          node = this.render(item);
          node.dataset.virtualKey = id;
          node.classList.add('chat-virtual-turn');
          this.nodes.set(id, node);
        }
        wanted.add(id);
        desired.push(node);
      } else skipped += height;
      offset += height;
    }
    spacer();
    for (const [id, node] of this.nodes) {
      if (!wanted.has(id)) { this.observer.unobserve(node); this.nodes.delete(id); }
    }
    // Keep mounted media/focus/streams in the document, and never collapse
    // total height to zero between frames (which would clamp scrollTop).
    const keep = new Set(desired);
    let next = this.root.firstElementChild;
    for (const node of desired) {
      // Replacement spacers are already inserted before this point. Retire
      // obsolete siblings without detaching the next retained media row; this
      // also preserves playback on browsers without Element.moveBefore.
      if (node.parentElement === this.root) {
        while (next && !keep.has(next as HTMLElement)) {
          const obsolete = next;
          next = next.nextElementSibling;
          obsolete.remove();
        }
      }
      if (node === next) { next = next.nextElementSibling; continue; }
      if (node.parentElement === this.root && 'moveBefore' in this.root) {
        this.root.moveBefore(node, next);
      } else this.root.insertBefore(node, next);
    }
    for (const node of Array.from(this.root.children)) if (!keep.has(node as HTMLElement)) node.remove();
    if (this.following) this.move(this.scroll.scrollHeight);
    else if (anchor?.isConnected && anchorTop !== undefined) {
      this.move(this.scroll.scrollTop + anchor.getBoundingClientRect().top - anchorTop);
    } else if (correction) this.move(oldScroll + correction);
    for (const node of this.nodes.values()) this.observer.observe(node);
  }

  destroy(): void {
    this.destroyed = true;
    cancelAnimationFrame(this.frame);
    this.observer.disconnect();
    this.scroll.removeEventListener('scroll', this.onScroll);
    this.scroll.style.scrollBehavior = this.previousScrollBehavior;
    window.removeEventListener('resize', this.schedule);
    this.nodes.clear();
    this.items = [];
    this.live.clear();
  }
}
