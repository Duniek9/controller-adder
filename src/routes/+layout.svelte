<script lang="ts">
  import { page } from '$app/state';
  import type { LayoutProps } from './$types';

  let { children }: LayoutProps = $props();
</script>

<header class="app-header">
  <nav class="main-nav" aria-label="Main navigation">
    <a
      href="/"
      class:current={page.url.pathname === '/'}
      aria-current={page.url.pathname === '/' ? 'page' : undefined}
    >Home</a>
    <a
      href="/effective"
      class:current={page.url.pathname.startsWith('/effective')}
      aria-current={page.url.pathname.startsWith('/effective') ? 'page' : undefined}
    >🎮 Effective Inputs</a>
  </nav>
</header>

{@render children()}

<style>
  .app-header {
    max-width: 800px;
    margin: 24px auto 0;
    padding: 0 20px;
  }

  .main-nav {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    padding: 8px;
    border-radius: 10px;
    background: #24242d;
  }

  .main-nav a {
    padding: 10px 16px;
    border-radius: 7px;
    color: #e8e8f0;
    text-decoration: none;
    font-weight: 600;
    transition: background-color 0.15s ease;
  }

  .main-nav a:hover { background: #373746; }
  .main-nav a.current { color: white; background: #6246ea; }
</style>
