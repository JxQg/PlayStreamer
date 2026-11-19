const toast = document.querySelector('.toast');
let toastTimer;
function notify(message) {
  toast.textContent = message;
  toast.classList.add('visible');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => toast.classList.remove('visible'), 3500);
}
for (const button of document.querySelectorAll('[data-copy]')) {
  button.addEventListener('click', async () => {
    try {
      await navigator.clipboard.writeText(button.dataset.copy);
      notify('播放地址已复制，可粘贴到 OBS 媒体源。');
    } catch {
      const address = document.querySelector('#playback-address');
      if (address) { const selection = window.getSelection(); const range = document.createRange(); range.selectNodeContents(address); selection.removeAllRanges(); selection.addRange(range); }
      notify('未能访问剪贴板，请选中地址后手动复制。');
    }
  });
}
const tabs = [...document.querySelectorAll('[role=tab]')];
function activateTab(tab) {
  for (const item of tabs) {
    const active = item === tab;
    item.setAttribute('aria-selected', String(active));
    item.tabIndex = active ? 0 : -1;
    document.getElementById(item.getAttribute('aria-controls')).hidden = !active;
  }
  const oopz = tab.id === 'tab-oopz';
  document.querySelector('#platform-output').textContent = oopz ? 'OOPZ 语音频道' : 'Bilibili 直播间';
  document.querySelector('#platform-hint').textContent = oopz ? '分享 OBS 投影窗口，并单独确认共享声音。' : '按平台要求配置 OBS 编码与推流。';
}
for (const [index, tab] of tabs.entries()) {
  tab.addEventListener('click', () => activateTab(tab));
  tab.addEventListener('keydown', event => {
    if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
    event.preventDefault();
    const target = event.key === 'Home' ? tabs[0] : event.key === 'End' ? tabs.at(-1) : tabs[(index + (event.key === 'ArrowRight' ? 1 : -1) + tabs.length) % tabs.length];
    activateTab(target); target.focus();
  });
}
const menu = document.querySelector('.menu-toggle');
const navigation = document.querySelector('#site-navigation');
function closeMenu() { menu.setAttribute('aria-expanded', 'false'); navigation.classList.remove('open'); menu.setAttribute('aria-label', '展开导航'); }
menu.addEventListener('click', () => { const open = menu.getAttribute('aria-expanded') !== 'true'; menu.setAttribute('aria-expanded', String(open)); menu.setAttribute('aria-label', open ? '收起导航' : '展开导航'); navigation.classList.toggle('open', open); });
for (const link of navigation.querySelectorAll('a')) link.addEventListener('click', closeMenu);
document.addEventListener('keydown', event => { if (event.key === 'Escape' && menu.getAttribute('aria-expanded') === 'true') { closeMenu(); menu.focus(); } });
