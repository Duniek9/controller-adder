<script lang="ts">
  import { page } from '$app/state';
  import { onMount } from 'svelte';
  import type { LayoutProps } from './$types';

  let { children }: LayoutProps = $props();
  let collapsed = $state(false);
  const links = [
    { href: '/', label: 'Home', icon: 'M3 10l9-7 9 7v10a1 1 0 0 1-1 1h-5v-7H9v7H4a1 1 0 0 1-1-1z' },
    { href: '/effective', label: 'Effective Inputs', icon: 'M6 12h4m-2-2v4m7-2h.01m3-2h.01M6 6h12c2 0 3 2 3.5 5l.5 5c0 2-2 3-4 1l-3-3H9l-3 3c-2 2-4 1-4-1l.5-5C3 8 4 6 6 6z' },
    { href: '/mapping', label: 'Controller Mapping', icon: 'M4 7h16M4 12h16M4 17h16M8 4v6m8-1v6M10 14v6' }
  ];

  function isCurrent(href: string) {
    return href === '/' ? page.url.pathname === '/' :
      page.url.pathname === href || page.url.pathname.startsWith(href + '/');
  }

  onMount(() => {
    try {
      const saved = localStorage.getItem('controller-adder:sidebar-collapsed');
      collapsed = saved === null ? window.innerWidth < 1000 : saved === 'true';
    } catch {
      collapsed = window.innerWidth < 1000;
    }
  });

  function toggleSidebar() {
    collapsed = !collapsed;
    try { localStorage.setItem('controller-adder:sidebar-collapsed', String(collapsed)); } catch {}
  }
</script>

<div class="app-shell" class:collapsed>
  <aside class="sidebar">
    <div class="brand">
      <svg class="brand-icon" width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="M6 7h12c2 0 3 2 3.5 5l.5 4c0 2-2 3-4 1l-3-3H9l-3 3c-2 2-4 1-4-1l.5-4C3 9 4 7 6 7zM6 11h4m-2-2v4m7-2h.01m3 1h.01"/>
      </svg>
      {#if !collapsed}<span>Controller Adder</span>{/if}
    </div>
    <button class="toggle" type="button" onclick={toggleSidebar}
      aria-label={collapsed ? 'Expand navigation' : 'Collapse navigation'}
      aria-expanded={!collapsed} aria-controls="main-navigation"
      title={collapsed ? 'Expand navigation' : 'Collapse navigation'}>
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="M4 5h16v14H4zM9 5v14"/>
        <path d={collapsed ? 'm13 9 3 3-3 3' : 'm16 9-3 3 3 3'}/>
      </svg>
      {#if !collapsed}<span>Collapse sidebar</span>{/if}
    </button>
    <nav id="main-navigation" aria-label="Main navigation">
      {#each links as link}
        <a href={link.href} class:current={isCurrent(link.href)}
          aria-current={isCurrent(link.href) ? 'page' : undefined}
          aria-label={link.label} title={collapsed ? link.label : undefined}>
          <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d={link.icon}/></svg>
          {#if !collapsed}<span>{link.label}</span>{/if}
        </a>
      {/each}
    </nav>
  </aside>
  <div class="page-content">
    {@render children()}
  </div>
</div>

<style>
  .app-shell { display:grid; grid-template-columns:224px minmax(0,1fr); height:100vh; height:100dvh; overflow:hidden; }
  .app-shell.collapsed { grid-template-columns:68px minmax(0,1fr); }
  .sidebar { display:flex; flex-direction:column; gap:1rem; padding:1rem .65rem; background:var(--app-surface); border-right:1px solid var(--app-border); overflow-y:auto; overflow-x:hidden; }
  .brand { display:flex; align-items:center; gap:.65rem; min-height:44px; padding:.2rem .45rem; font-weight:700; font-size:.95rem; }
  .brand-icon { color:var(--app-accent); flex-shrink:0; }
  nav { display:grid; gap:.4rem; }
  nav a,.toggle { display:flex; align-items:center; gap:.7rem; min-height:44px; padding:.65rem .75rem; border-radius:8px; color:var(--app-muted); text-decoration:none; font-size:.88rem; font-weight:600; }
  nav a svg,.toggle svg { flex-shrink:0; }
  nav a:hover,.toggle:hover { background:var(--app-surface-raised); color:var(--app-text); }
  nav a.current { background:var(--app-primary); color:#fff; }
  .toggle { background:transparent; border:1px solid var(--app-border); cursor:pointer; text-align:left; width:100%; }
  .collapsed .brand,.collapsed nav a,.collapsed .toggle { justify-content:center; padding-left:0; padding-right:0; }
  .page-content { min-width:0; min-height:0; overflow:auto; }
  @media (max-width:520px) {
    .app-shell:not(.collapsed) { grid-template-columns:168px minmax(0,1fr); }
    .brand { font-size:.8rem; }
  }
</style>
