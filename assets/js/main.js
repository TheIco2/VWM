// ============================================================
//  VEIL Window Manager — Site JS
// ============================================================

(function () {
  'use strict';

  /* ── Nav scroll effect ─────────────────────────────────── */
  const navbar = document.querySelector('.navbar');
  if (navbar) {
    const onScroll = () => {
      navbar.style.background = window.scrollY > 20
        ? 'rgba(13, 15, 20, 0.97)'
        : 'rgba(13, 15, 20, 0.82)';
    };
    window.addEventListener('scroll', onScroll, { passive: true });
  }

  /* ── Mobile nav toggle ─────────────────────────────────── */
  const toggle = document.querySelector('.nav-toggle');
  const mobileMenu = document.querySelector('.nav-mobile');
  if (toggle && mobileMenu) {
    toggle.addEventListener('click', () => {
      const open = mobileMenu.classList.toggle('open');
      toggle.setAttribute('aria-expanded', String(open));
    });
    // close on link click
    mobileMenu.querySelectorAll('a').forEach(a => {
      a.addEventListener('click', () => mobileMenu.classList.remove('open'));
    });
  }

  /* ── Active nav link on scroll ─────────────────────────── */
  const sections = document.querySelectorAll('section[id]');
  const navLinks = document.querySelectorAll('.nav-links a[href^="#"], .nav-mobile a[href^="#"]');
  if (sections.length && navLinks.length) {
    const io = new IntersectionObserver((entries) => {
      entries.forEach(entry => {
        if (entry.isIntersecting) {
          const id = entry.target.id;
          navLinks.forEach(a => {
            a.classList.toggle('active', a.getAttribute('href') === `#${id}`);
          });
        }
      });
    }, { rootMargin: '-40% 0px -55% 0px' });
    sections.forEach(s => io.observe(s));
  }

  /* ── Layout tabs ───────────────────────────────────────── */
  const tabButtons = document.querySelectorAll('.layout-tab');
  const tabPanels  = document.querySelectorAll('.layout-panel');
  tabButtons.forEach(btn => {
    btn.addEventListener('click', () => {
      const target = btn.dataset.tab;
      tabButtons.forEach(b => b.classList.toggle('active', b === btn));
      tabPanels.forEach(p => p.classList.toggle('active', p.id === target));
    });
  });

  /* ── Copy buttons ──────────────────────────────────────── */
  document.querySelectorAll('.copy-btn').forEach(btn => {
    btn.addEventListener('click', () => {
      const block = btn.closest('.code-block');
      const text  = block?.querySelector('pre')?.textContent ?? '';
      navigator.clipboard.writeText(text).then(() => {
        btn.textContent = 'Copied!';
        btn.classList.add('copied');
        setTimeout(() => {
          btn.textContent = 'Copy';
          btn.classList.remove('copied');
        }, 2000);
      });
    });
  });

  /* ── Docs sidebar active link ──────────────────────────── */
  const docsSections = document.querySelectorAll('.docs-content section[id]');
  const docsLinks    = document.querySelectorAll('.docs-nav a[href^="#"]');
  if (docsSections.length && docsLinks.length) {
    const io2 = new IntersectionObserver((entries) => {
      entries.forEach(entry => {
        if (entry.isIntersecting) {
          const id = entry.target.id;
          docsLinks.forEach(a => {
            a.classList.toggle('active', a.getAttribute('href') === `#${id}`);
          });
        }
      });
    }, { rootMargin: '-10% 0px -80% 0px' });
    docsSections.forEach(s => io2.observe(s));
  }

  /* ── Scroll-in animation ───────────────────────────────── */
  const fadeEls = document.querySelectorAll('.feature-card, .install-step, .layout-panel');
  if ('IntersectionObserver' in window && fadeEls.length) {
    fadeEls.forEach(el => {
      el.style.opacity = '0';
      el.style.transform = 'translateY(18px)';
      el.style.transition = 'opacity 0.45s ease, transform 0.45s ease';
    });
    const io3 = new IntersectionObserver((entries) => {
      entries.forEach(entry => {
        if (entry.isIntersecting) {
          entry.target.style.opacity = '1';
          entry.target.style.transform = 'translateY(0)';
          io3.unobserve(entry.target);
        }
      });
    }, { rootMargin: '0px 0px -60px 0px' });
    fadeEls.forEach(el => io3.observe(el));
  }
})();
