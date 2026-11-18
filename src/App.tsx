import { useCallback, useEffect, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import packageInfo from '../package.json';
import { Activity, ArrowLeft, ArrowRight, BookOpen, Cable, Check, CheckCircle2, ChevronRight, CircleAlert, Copy, Download, ExternalLink, Gamepad2, HelpCircle, Info, LoaderCircle, Maximize2, Minus, Monitor, Moon, Network, Play, Power, Radio, RefreshCw, Settings2, ShieldCheck, Square, Sun, X } from 'lucide-react';
import { command, desktop, errorMessage } from './api';
import type { AppConfig, DiagnosticReport, NetworkInterface, ServiceStatus, Snapshot, Theme, UpdateInfo } from './api';
import { guides } from './guides';
import { GuideIllustration } from './GuideIllustration';
import type { GuideId } from './guides';

type Page = 'home' | 'guides' | 'settings';
type Notice = { kind: 'success' | 'error' | 'info'; text: string };
type SettingsDraft = { networkInterfaceId: string; ps5Ip: string; upstreamDns: string; theme: Theme };
const emptyDraft: SettingsDraft = { networkInterfaceId: '', ps5Ip: '', upstreamDns: '', theme: 'system' };
const statusLabels: Record<ServiceStatus, string> = { idle: '尚未开启', starting: '正在启动', waiting: '等待主机开播', receiving: '正在接收', dnsOnly: '仅 DNS 运行中', networkChanged: '网络发生变化', mediaError: '媒体服务异常', suspended: '接收已暂停' };

function draftFrom(config: AppConfig, interfaces: NetworkInterface[]): SettingsDraft {
  const selected = interfaces.find(item => item.id === config.networkInterfaceId) ?? interfaces.find(item => item.eligible);
  return {
    networkInterfaceId: selected?.id ?? '',
    ps5Ip: config.ps5Ip,
    upstreamDns: config.upstreamDns.length ? config.upstreamDns.join(', ') : (selected?.dnsServers.filter(item => validIpv4(item) && item !== selected.ipv4 && !item.startsWith('127.')).join(', ') ?? ''),
    theme: config.theme,
  };
}
function validIpv4(value: string): boolean {
  const parts = value.split('.');
  return parts.length === 4 && parts.every(part => /^\d{1,3}$/.test(part) && Number(part) <= 255) && !['0.0.0.0', '255.255.255.255'].includes(value);
}
function duration(seconds: number): string {
  const hours = Math.floor(seconds / 3600);
  return `${hours ? `${hours}:` : ''}${String(Math.floor(seconds / 60) % 60).padStart(2, '0')}:${String(seconds % 60).padStart(2, '0')}`;
}

export default function App() {
  const [page, setPage] = useState<Page>('home');
  const [guide, setGuide] = useState<GuideId>('ps5');
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [interfaces, setInterfaces] = useState<NetworkInterface[]>([]);
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [draft, setDraft] = useState<SettingsDraft>(emptyDraft);
  const [busy, setBusy] = useState<string | null>(null);
  const [loading, setLoading] = useState(desktop);
  const [notice, setNotice] = useState<Notice | null>(null);
  const [wizard, setWizard] = useState<number | null>(null);
  const [diagnostics, setDiagnostics] = useState<DiagnosticReport | null>(null);
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const [exportPath, setExportPath] = useState<string | null>(null);
  const [previewTheme, setPreviewTheme] = useState<Theme>('system');
  const busyRef = useRef(false);
  const theme = config?.theme ?? previewTheme;
  const selectedNetwork = interfaces.find(item => item.id === draft.networkInterfaceId);
  const dnsAddress = snapshot?.lanIp ?? interfaces.find(item => item.id === config?.networkInterfaceId)?.ipv4;
  const active = !!snapshot && ['starting', 'waiting', 'receiving'].includes(snapshot.status);
  const networkLocked = !!snapshot?.dnsRunning || active;

  const task = useCallback(async (name: string, work: () => Promise<void>) => {
    if (busyRef.current) return;
    busyRef.current = true; setBusy(name); setNotice(null);
    try { await work(); } catch (error) { setNotice({ kind: 'error', text: errorMessage(error) }); }
    finally { busyRef.current = false; setBusy(null); }
  }, []);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);

  useEffect(() => {
    if (!desktop) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    async function initialize() {
      try {
        unlisten = await listen<Snapshot>('service-state', event => { if (!disposed) setSnapshot(event.payload); });
        if (disposed) { unlisten(); return; }
        const [configResult, networkResult, snapshotResult] = await Promise.allSettled([command<AppConfig>('get_config'), command<NetworkInterface[]>('list_network_interfaces'), command<Snapshot>('get_snapshot')]);
        if (disposed) return;
        const adapters = networkResult.status === 'fulfilled' ? networkResult.value : [];
        setInterfaces(adapters);
        if (snapshotResult.status === 'fulfilled') setSnapshot(snapshotResult.value);
        if (configResult.status === 'fulfilled') {
          const storedConfig = configResult.value;
          setConfig(storedConfig); setDraft(draftFrom(storedConfig, adapters));
          if (!storedConfig.setupCompleted) setWizard(0);
        }
        const failures: string[] = [];
        if (configResult.status === 'rejected') failures.push(`读取配置失败：${errorMessage(configResult.reason)}`);
        if (networkResult.status === 'rejected') failures.push(`读取网卡失败，可在设置中刷新重试：${errorMessage(networkResult.reason)}`);
        if (snapshotResult.status === 'rejected') failures.push(`读取接收状态失败：${errorMessage(snapshotResult.reason)}`);
        if (failures.length) setNotice({ kind: 'error', text: failures.join('；') });
      } catch (error) { if (!disposed) setNotice({ kind: 'error', text: `无法连接桌面服务：${errorMessage(error)}` }); }
      finally { if (!disposed) setLoading(false); }
    }
    void initialize();
    return () => { disposed = true; unlisten?.(); };
  }, []);

  useEffect(() => {
    if (!desktop || !config) return;
    let disposed = false;
    let inFlight = false;
    const timer = window.setInterval(async () => {
      if (inFlight) return;
      inFlight = true;
      try { const current = await command<Snapshot>('get_snapshot'); if (!disposed) setSnapshot(current); }
      catch (error) { if (!disposed) setNotice({ kind: 'error', text: `状态同步失败：${errorMessage(error)}` }); }
      finally { inFlight = false; }
    }, 3000);
    return () => { disposed = true; window.clearInterval(timer); };
  }, [config]);

  async function saveDraft(completed: boolean) {
    if (!draft.networkInterfaceId || !selectedNetwork?.eligible) throw new Error('请选择与 PS5 相连的可用局域网网卡。');
    if (!validIpv4(draft.ps5Ip.trim())) throw new Error('请填写有效的 PS5 IPv4 地址，例如 192.168.1.120。');
    const enteredDns = draft.upstreamDns.split(/[,，;；\s]+/).filter(Boolean);
    const dns = enteredDns.length ? enteredDns : selectedNetwork.dnsServers.filter(item => validIpv4(item) && item !== selectedNetwork.ipv4 && !item.startsWith('127.'));
    if (!dns.length || !dns.every(validIpv4)) throw new Error('未检测到有效上游 DNS，请填写可用的 IPv4 DNS 地址，多个地址用逗号分隔。');
    const next: AppConfig = { networkInterfaceId: draft.networkInterfaceId, ps5Ip: draft.ps5Ip.trim(), upstreamDns: dns, theme: draft.theme, setupCompleted: completed };
    await command<void>('save_config', { config: next });
    setConfig(next); setDraft(draftFrom(next, interfaces));
    setSnapshot(await command<Snapshot>('get_snapshot'));
  }

  function showGuide(id: GuideId) { setGuide(id); setPage('guides'); }
  function copy(value: string, label: string) { void task('copy', async () => { await writeText(value); setNotice({ kind: 'success', text: `已复制${label}` }); }); }
  function external(url: string) {
    if (!desktop) { window.open(url, '_blank', 'noopener,noreferrer'); return; }
    void task('external', () => command<void>('open_external', { url }));
  }
  function toggleReceiving() {
    if (!config?.setupCompleted && !active) { setWizard(0); return; }
    void task('receiving', async () => { setSnapshot(await command<Snapshot>(active ? 'stop_receiving' : 'start_receiving')); });
  }
  function windowAction(action: 'minimize' | 'maximize' | 'hide') {
    if (!desktop) return;
    const win = getCurrentWindow();
    void task('window', () => action === 'minimize' ? win.minimize() : action === 'maximize' ? win.toggleMaximize() : win.hide());
  }
  function chooseNetwork(id: string) {
    const adapter = interfaces.find(item => item.id === id);
    setDraft(current => ({ ...current, networkInterfaceId: id, upstreamDns: adapter?.dnsServers.filter(item => validIpv4(item) && item !== adapter.ipv4 && !item.startsWith('127.')).join(', ') ?? '' }));
  }
  function refreshInterfaces() {
    void task('interfaces', async () => { setInterfaces(await command<NetworkInterface[]>('list_network_interfaces')); setNotice({ kind: 'success', text: '已刷新可用网卡' }); });
  }
  const networkFields = (id: string) => (
    <div className="form-fields">
      <label htmlFor={`${id}-network`}>电脑网卡<span>选择与 PS5 连接到同一路由器的网卡</span></label>
      <div className="field-with-action"><select id={`${id}-network`} value={draft.networkInterfaceId} onChange={event => chooseNetwork(event.target.value)} disabled={!desktop || !!busy || networkLocked}>
        <option value="">{interfaces.length ? '请选择网卡' : '未发现可用网卡'}</option>
        {interfaces.map(item => <option key={item.id} value={item.id} disabled={!item.eligible}>{item.name} — {item.ipv4}{item.eligible ? '' : '（不可用）'}</option>)}
      </select><button className="icon-button bordered" aria-label="刷新网卡" title="刷新网卡" onClick={refreshInterfaces} disabled={!desktop || !!busy || networkLocked}><RefreshCw size={16} className={busy === 'interfaces' ? 'spin' : ''} /></button></div>
      <label htmlFor={`${id}-ps5`}>PS5 的 IPv4 地址<span>在 PS5 的「查看连接状态」中查找</span></label>
      <input id={`${id}-ps5`} inputMode="decimal" placeholder="例如 192.168.1.120" value={draft.ps5Ip} onChange={event => setDraft({ ...draft, ps5Ip: event.target.value })} disabled={!desktop || !!busy || networkLocked} autoComplete="off" spellCheck={false} />
      <label htmlFor={`${id}-dns`}>上游 DNS<span>用于解析正常联网域名，多个地址以逗号分隔</span></label>
      <input id={`${id}-dns`} placeholder="例如 223.5.5.5, 119.29.29.29" value={draft.upstreamDns} onChange={event => setDraft({ ...draft, upstreamDns: event.target.value })} disabled={!desktop || !!busy || networkLocked} autoComplete="off" spellCheck={false} />
      {networkLocked && <p className="field-note"><Info size={14} />DNS 正在为主机联网服务。修改网络前，请将 PS5 DNS 恢复自动，再完全退出并重开应用。</p>}
    </div>
  );

  return (
    <div className="app-shell">
      <header className="titlebar">
        <div className="titlebar-brand" data-tauri-drag-region><img src="/brand/playstreamer.svg" alt="" /><span data-tauri-drag-region>PlayStreamer</span></div>
        <div className="titlebar-drag" data-tauri-drag-region />
        {!desktop && <span className="preview-chip">界面预览</span>}
        <div className="window-controls"><button onClick={() => windowAction('minimize')} disabled={!desktop} aria-label="最小化" title="最小化"><Minus size={15} /></button><button onClick={() => windowAction('maximize')} disabled={!desktop} aria-label="最大化或还原" title="最大化或还原"><Maximize2 size={13} /></button><button className="window-close" onClick={() => windowAction('hide')} disabled={!desktop} aria-label="隐藏到系统托盘" title="隐藏到系统托盘，继续接收"><X size={17} /></button></div>
      </header>
      <div className="app-body">
        <aside className="sidebar">
          <div className="sidebar-caption">让精彩继续</div>
          <nav aria-label="主导航">
            <NavButton active={page === 'home'} onClick={() => setPage('home')} icon={<Radio size={19} />} label="直播接收" />
            <NavButton active={page === 'guides'} onClick={() => setPage('guides')} icon={<BookOpen size={19} />} label="使用教程" />
            <NavButton active={page === 'settings'} onClick={() => setPage('settings')} icon={<Settings2 size={19} />} label="设置与诊断" />
          </nav>
          <div className="sidebar-bottom">
            <div className="sidebar-status"><span className={`status-dot ${snapshot?.status === 'receiving' ? 'live' : snapshot?.dnsRunning ? 'ready' : ''}`} /><span>{!desktop ? '桌面服务未连接' : loading ? '连接桌面服务' : snapshot ? statusLabels[snapshot.status] : '桌面服务异常'}</span></div>
            <button className="sidebar-help" onClick={() => showGuide('troubleshooting')}><HelpCircle size={16} />连接遇到问题<ChevronRight size={14} /></button>
            <div className="sidebar-version"><span>PlayStreamer {packageInfo.version}</span><button onClick={() => void task('quit', () => command<void>('quit_app'))} disabled={!desktop || !!busy} title="完全退出" aria-label="完全退出 PlayStreamer"><Power size={14} /></button></div>
          </div>
        </aside>
        <main className="main-content" id="main-content">
          {!desktop && <div className="preview-banner"><Info size={17} /><span>当前为浏览器预览，未连接桌面服务。教程可阅读；接收、复制和网络设置需在桌面客户端中使用。</span></div>}
          {loading ? <div className="loading-screen"><LoaderCircle className="spin" size={28} /><p>正在读取连接设置…</p></div> : <>
            {page === 'home' && <div className="page home-page">
              <PageHeader title="直播接收" description="从 PlayStation 到 OBS，再到你的观众。"><button className="button subtle" onClick={() => desktop ? setWizard(0) : showGuide('ps5')}><Cable size={16} />连接指引</button></PageHeader>
              <section className={`receiver-panel ${snapshot?.status === 'receiving' ? 'is-live' : ''}`} aria-label="接收状态">
                <div className="receiver-top"><div className="receiver-symbol"><Radio size={28} strokeWidth={1.5} /></div><span className={`status-pill ${snapshot?.status === 'receiving' ? 'live' : ''}`}><span className="status-dot" />{!desktop ? '未连接桌面服务' : snapshot ? statusLabels[snapshot.status] : '状态不可用'}</span></div>
                <h2>{snapshot?.status === 'receiving' ? '你的游戏，已连接。' : active ? '准备好了，去 PS5 开播吧。' : '把主机画面，带到这里。'}</h2>
                <p className="receiver-description">{!desktop ? '在桌面客户端中完成连接设置，即可接收 PS5 的直播流。' : snapshot?.message || '先完成局域网设置，再开启直播接收。'}</p>
                <div className="receiver-actions"><button className={`button ${active ? 'secondary' : 'primary'}`} onClick={toggleReceiving} disabled={!desktop || !!busy || !snapshot}>{busy === 'receiving' ? <LoaderCircle size={17} className="spin" /> : active ? <Square size={15} /> : <Play size={17} fill="currentColor" />}{busy === 'receiving' ? '正在处理…' : active ? '停止接收' : snapshot?.status === 'dnsOnly' ? '重新开启接收' : config?.setupCompleted ? '开启接收' : '设置并开启接收'}</button><span className="receiver-hint">{active ? '关闭窗口后仍在托盘运行' : '仅接收配置的 PS5 直播'}</span></div>
                <div className="signal-route"><div><Gamepad2 size={19} /><span>PlayStation 5</span><i className={snapshot?.status === 'receiving' ? 'connected' : ''} /></div><span className={`route-line ${snapshot?.status === 'receiving' ? 'connected' : ''}`} /><div className="route-local"><img src="/brand/playstreamer.svg" alt="" /><span>PlayStreamer</span></div><span className={`route-line ${snapshot && snapshot.stream.readers > 0 ? 'connected' : ''}`} /><div><Monitor size={18} /><span>OBS / 播放器</span><i className={snapshot && snapshot.stream.readers > 0 ? 'connected' : ''} /></div></div>
              </section>
              {snapshot?.lastError && <div className="inline-alert error"><CircleAlert size={17} /><span>{snapshot.lastError}</span><button onClick={() => setPage('settings')}>运行诊断<ChevronRight size={14} /></button></div>}
              <div className="connection-grid">
                <section className="connection-card"><div className="section-heading"><h3><Gamepad2 size={18} />PS5 首选 DNS</h3><button className="text-button" onClick={() => showGuide('ps5')}>如何设置<ChevronRight size={13} /></button></div><div className="address-row"><code>{dnsAddress ?? '等待选择网卡'}</code><CopyButton disabled={!desktop || !dnsAddress || !!busy} onClick={() => dnsAddress && copy(dnsAddress, 'PS5 首选 DNS')} label="复制 DNS 地址" /></div><p>在主机网络设置中，填入这台电脑的地址。</p></section>
                <section className="connection-card"><div className="section-heading"><h3><Monitor size={18} />OBS 播放地址</h3><button className="text-button" onClick={() => showGuide('obs')}>接入 OBS<ChevronRight size={13} /></button></div><div className="address-row compact"><code>{snapshot?.playbackUrl ?? '等待桌面服务连接'}</code><CopyButton disabled={!desktop || !snapshot?.playbackUrl || !!busy} onClick={() => snapshot && copy(snapshot.playbackUrl, 'OBS 播放地址')} label="复制 RTMP 播放地址" /></div><p>添加到 OBS「媒体源」的输入地址。</p></section>
              </div>
              <section className="stream-stats" aria-label="实际接收统计"><Stat label="接收码率" value={snapshot?.status === 'receiving' ? `${snapshot.stream.bitrateKbps.toLocaleString()} kbps` : '—'} /><Stat label="已连接" value={snapshot?.status === 'receiving' ? duration(snapshot.stream.connectedSeconds) : '—'} /><Stat label="媒体轨道" value={snapshot?.stream.tracks.length ? snapshot.stream.tracks.join(' / ') : '—'} /><Stat label="正在读取" value={snapshot ? `${snapshot.stream.readers} 个客户端` : '—'} /></section>
              <div className="platform-links"><span>下一步，把画面分享出去</span><button onClick={() => showGuide('bilibili')}>Bilibili 直播<ArrowRight size={15} /></button><button onClick={() => showGuide('oopz')}>OOPZ 频道分享<ArrowRight size={15} /></button></div>
            </div>}
            {page === 'guides' && <div className="page guides-page"><PageHeader title="使用教程" description="从第一次连接，到顺利开播。所有教程均可离线阅读。" /><div className="guide-layout"><nav className="guide-nav" aria-label="教程目录">{guides.map(item => <button key={item.id} className={guide === item.id ? 'selected' : ''} onClick={() => setGuide(item.id)}><span>{item.id === 'ps5' ? '连接 PS5' : item.title}</span><ChevronRight size={14} /></button>)}<div className="guide-note"><BookOpen size={16} /><span>平台界面可能变化，实际入口以当前版本为准。</span></div></nav><article className="guide-article">{guides.filter(item => item.id === guide).map(item => <div key={item.id}><div className="guide-meta"><span>{item.time}</span><span>离线教程</span></div><h2>{item.title}</h2><p className="guide-subtitle">{item.subtitle}</p><div className="guide-sections">{(item.id === 'ps5' || item.id === 'obs') && <figure className="guide-figure"><GuideIllustration kind={item.id} /><figcaption>设置示意图，实际界面以当前软件版本为准。</figcaption></figure>}{item.sections.map((section, index) => <section key={section.title}><h3><span>{index + 1}</span>{section.title}</h3>{section.paragraphs.map(paragraph => <p key={paragraph}>{paragraph}</p>)}{section.note && <aside className="guide-callout"><Info size={16} /><p>{section.note}</p></aside>}</section>)}</div>{item.links && <div className="guide-external-links">{item.links.map(link => <button key={link.url} className="button secondary" onClick={() => external(link.url)}><ExternalLink size={15} />{link.label}</button>)}</div>}<div className="guide-footer"><CheckCircle2 size={17} /><span>准备好了？返回首页开启接收。</span><button className="text-button" onClick={() => setPage('home')}>返回首页<ArrowRight size={15} /></button></div></div>)}</article></div></div>}
            {page === 'settings' && <div className="page settings-page"><PageHeader title="设置与诊断" description="连接配置保存在本机。这里也能检查当前的运行状态。" /><div className="settings-columns"><section className="settings-section"><div className="section-heading"><h2><Network size={19} />连接设置</h2></div>{networkFields('settings')}<button className="button primary save-button" disabled={!desktop || !!busy || networkLocked} onClick={() => void task('save', async () => { await saveDraft(config?.setupCompleted ?? false); setNotice({ kind: 'success', text: '连接设置已保存' }); })}>{busy === 'save' ? <LoaderCircle size={16} className="spin" /> : <Check size={16} />}保存连接设置</button></section><div className="settings-side"><section className="settings-section"><h2>外观</h2><p className="setting-description">选择适合你的界面主题</p><div className="theme-control">{([{ value: 'system', label: '跟随系统', icon: <Monitor size={17} /> }, { value: 'light', label: '浅色', icon: <Sun size={17} /> }, { value: 'dark', label: '深色', icon: <Moon size={17} /> }] as const).map(item => <button key={item.value} className={theme === item.value ? 'selected' : ''} disabled={!!busy} onClick={() => { if (!desktop) { setPreviewTheme(item.value); return; } void task('theme', async () => { if (!config) throw new Error('尚未读取桌面设置。'); const next = { ...config, theme: item.value }; await command<void>('save_config', { config: next }); setConfig(next); setDraft(current => ({ ...current, theme: item.value })); }); }}>{item.icon}<span>{item.label}</span></button>)}</div></section><section className="settings-section"><h2>Windows 防火墙</h2><p className="setting-description">允许 PS5 访问这台电脑的接收服务。系统可能要求管理员确认。</p><button className="button secondary" disabled={!desktop || !!busy} onClick={() => void task('firewall', async () => { const result = await command<string>('configure_firewall'); setNotice({ kind: 'success', text: result }); })}>{busy === 'firewall' ? <LoaderCircle size={16} className="spin" /> : <ShieldCheck size={16} />}配置防火墙</button></section><section className="settings-section update-section"><div><h2>版本更新</h2><p className="setting-description">当前版本 {packageInfo.version}</p></div><button className="icon-button bordered" disabled={!desktop || !!busy} aria-label="检查版本更新" title="检查版本更新" onClick={() => void task('update', async () => { const result = await command<UpdateInfo>('check_updates'); setUpdate(result); setNotice({ kind: 'info', text: result.updateAvailable ? `发现新版本 ${result.latestVersion}` : `已是最新版本 ${result.currentVersion}` }); })}><RefreshCw size={16} className={busy === 'update' ? 'spin' : ''} /></button>{update?.updateAvailable && <button className="text-button update-link" onClick={() => external(update.releaseUrl)}>下载 {update.latestVersion}<ExternalLink size={14} /></button>}</section></div></div><section className="diagnostic-section"><div className="section-heading"><div><h2><Activity size={19} />运行诊断</h2><p className="setting-description">检查网络、端口和接收服务，定位未收到画面的原因。</p></div><div className="button-group"><button className="button secondary" disabled={!desktop || !!busy} onClick={() => void task('export', async () => { const path = await command<string>('export_diagnostics'); setExportPath(path); setNotice({ kind: 'success', text: '诊断文件已导出，路径显示在检查结果下方。' }); })}><Download size={15} />导出</button><button className="button primary" disabled={!desktop || !!busy} onClick={() => void task('diagnostics', async () => { setDiagnostics(await command<DiagnosticReport>('run_diagnostics')); })}>{busy === 'diagnostics' ? <LoaderCircle size={16} className="spin" /> : <Activity size={16} />}开始检查</button></div></div>{diagnostics ? <div className="diagnostic-results">{diagnostics.checks.map(item => <div className={`diagnostic-result ${item.status}`} key={item.id}>{item.status === 'pass' ? <CheckCircle2 size={19} /> : <CircleAlert size={19} />}<div><h3>{item.title}<span>{item.status === 'pass' ? '通过' : item.status === 'warn' ? '需要留意' : '需处理'}</span></h3><p>{item.detail}</p></div></div>)}</div> : <div className="diagnostic-empty"><ShieldCheck size={25} strokeWidth={1.4} /><p>{desktop ? '尚未运行检查' : '诊断需要连接桌面客户端'}</p></div>}{exportPath && <div className="export-path"><CheckCircle2 size={16} /><span>已导出至 <code>{exportPath}</code></span><CopyButton label="复制诊断文件路径" disabled={!!busy} onClick={() => copy(exportPath, '诊断文件路径')} /></div>}<div className="diagnostic-counters"><span>DNS 查询 <strong>{snapshot?.dnsQueries ?? '—'}</strong></span><span>直播域名命中 <strong>{snapshot?.dnsIntercepts ?? '—'}</strong></span><span>配置的 PS5 <strong>{config?.ps5Ip || '尚未设置'}</strong></span></div></section></div>}
          </>}
        </main>
      </div>
      {notice && <div className={`toast ${notice.kind}`} role={notice.kind === 'error' ? 'alert' : 'status'}>{notice.kind === 'success' ? <CheckCircle2 size={18} /> : notice.kind === 'error' ? <CircleAlert size={18} /> : <Info size={18} />}<span>{notice.text}</span><button aria-label="关闭提示" onClick={() => setNotice(null)}><X size={16} /></button></div>}
      {wizard !== null && <SetupWizard step={wizard} busy={busy} active={active} config={config} snapshot={snapshot} selectedNetwork={selectedNetwork} fields={networkFields('wizard')} onClose={() => setWizard(null)} onBack={() => setWizard(Math.max(0, wizard - 1))} onNext={() => void task('wizard', async () => { if (wizard === 0) await saveDraft(config?.setupCompleted ?? false); if (wizard === 2) { await saveDraft(true); setWizard(null); setNotice({ kind: 'success', text: '设置已完成。主机开播后，首页会显示实际接收状态。' }); } else setWizard(wizard + 1); })} onStart={() => void task('receiving', async () => { setSnapshot(await command<Snapshot>('start_receiving')); })} onCopy={copy} onGuide={id => { setWizard(null); showGuide(id); }} />}
    </div>
  );
}

function NavButton({ active, onClick, icon, label }: { active: boolean; onClick: () => void; icon: ReactNode; label: string }) { return <button className={`nav-button ${active ? 'active' : ''}`} aria-current={active ? 'page' : undefined} onClick={onClick}>{icon}<span>{label}</span></button>; }
function PageHeader({ title, description, children }: { title: string; description: string; children?: ReactNode }) { return <div className="page-header"><div><h1>{title}</h1><p>{description}</p></div>{children}</div>; }
function CopyButton({ disabled, onClick, label }: { disabled: boolean; onClick: () => void; label: string }) { return <button className="copy-button" disabled={disabled} onClick={onClick} aria-label={label} title={label}><Copy size={16} /></button>; }
function Stat({ label, value }: { label: string; value: string }) { return <div className="stat"><span>{label}</span><strong>{value}</strong></div>; }

function SetupWizard({ step, busy, active, config, snapshot, selectedNetwork, fields, onClose, onBack, onNext, onStart, onCopy, onGuide }: { step: number; busy: string | null; active: boolean; config: AppConfig | null; snapshot: Snapshot | null; selectedNetwork: NetworkInterface | undefined; fields: ReactNode; onClose: () => void; onBack: () => void; onNext: () => void; onStart: () => void; onCopy: (value: string, label: string) => void; onGuide: (id: GuideId) => void }) {
  const modalRef = useRef<HTMLDivElement>(null);
  const closeRef = useRef(onClose);
  useEffect(() => { closeRef.current = onClose; }, [onClose]);
  useEffect(() => {
    const priorFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    modalRef.current?.focus();
    function keydown(event: KeyboardEvent) {
      if (event.key === 'Escape') { closeRef.current(); return; }
      if (event.key !== 'Tab') return;
      const focusable = modalRef.current?.querySelectorAll<HTMLElement>('button:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex="0"]');
      if (!focusable?.length) return;
      const first = focusable[0]; const last = focusable[focusable.length - 1];
      if (event.shiftKey && (document.activeElement === first || document.activeElement === modalRef.current)) { event.preventDefault(); last.focus(); }
      else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
    }
    document.addEventListener('keydown', keydown);
    return () => { document.removeEventListener('keydown', keydown); priorFocus?.focus(); };
  }, []);
  const dnsAddress = snapshot?.lanIp ?? selectedNetwork?.ipv4;
  return <div className="modal-backdrop"><div ref={modalRef} className="setup-modal" role="dialog" aria-modal="true" aria-labelledby="wizard-title" tabIndex={-1}><div className="wizard-heading"><img src="/brand/playstreamer.svg" alt="" /><div><h2 id="wizard-title">连接你的 PlayStation</h2><p>完成一次设置，之后开播更轻松。</p></div><button className="icon-button" aria-label="稍后设置" onClick={onClose}><X size={20} /></button></div><ol className="wizard-progress">{['选择网络', '设置主机', '接入 OBS'].map((title, index) => <li className={index === step ? 'current' : index < step ? 'done' : ''} key={title}><span>{index < step ? <Check size={13} /> : index + 1}</span>{title}</li>)}</ol><div className="wizard-body">{step === 0 && <><h3>先让电脑和主机找到彼此</h3><p className="wizard-intro">将 PS5 与电脑连接到同一路由器。建议使用有线网络。</p>{fields}</>}{step === 1 && <><h3>把主机的首选 DNS 指向电脑</h3><p className="wizard-intro">PS5「设置 → 网络 → 设置互联网连接 → 当前网络 → 高级设置」，将 DNS 改为手动。</p><div className="wizard-address"><span>PS5 首选 DNS</span><div><code>{dnsAddress || '未获得电脑地址'}</code><CopyButton disabled={!dnsAddress || !!busy} onClick={() => dnsAddress && onCopy(dnsAddress, 'PS5 首选 DNS')} label="复制 PS5 首选 DNS" /></div></div><div className="guide-callout"><Info size={17} /><p>备用 DNS 按主机界面允许留空或设为 0.0.0.0，避免公共 DNS 绕过接收。退出应用前，请将 PS5 DNS 恢复自动。</p></div><div className="wizard-start"><div><strong>{active ? '接收服务已开启' : '先开启电脑接收服务'}</strong><p>{snapshot?.status === 'receiving' ? '已实际收到主机直播流。' : '随后在 PS5 关联 Twitch，并发起播送。'}</p></div><button className="button primary" disabled={active || !!busy} onClick={onStart}>{active ? <Check size={16} /> : <Play size={15} />}{active ? '已开启' : '开启接收'}</button></div><button className="text-button" onClick={() => onGuide('ps5')}>查看完整 PS5 设置教程<ChevronRight size={14} /></button></>}{step === 2 && <><h3>让 OBS 读取直播画面</h3><p className="wizard-intro">在 OBS「来源」中添加「媒体源」，取消「本地文件」，把下面的地址填入「输入」。</p><div className="wizard-address"><span>同一台电脑上的 OBS 播放地址</span><div><code className="small-code">{snapshot?.playbackUrl || '未获得播放地址'}</code><CopyButton disabled={!snapshot?.playbackUrl || !!busy} onClick={() => snapshot && onCopy(snapshot.playbackUrl, 'OBS 播放地址')} label="复制 RTMP 播放地址" /></div></div><div className="setup-checklist"><p><CheckCircle2 size={17} />已保存主机地址 <strong>{config?.ps5Ip || '未设置'}</strong></p><p><Info size={17} />主机开始播送后，OBS 才会出现画面。</p><p><Info size={17} />画面混合、麦克风与平台推流由 OBS 完成。</p></div><button className="text-button" onClick={() => onGuide('obs')}>查看完整 OBS 教程<ChevronRight size={14} /></button></>}</div><footer className="wizard-footer"><button className="button subtle" onClick={step === 0 ? onClose : onBack} disabled={!!busy}>{step > 0 && <ArrowLeft size={15} />}{step === 0 ? '稍后设置' : '上一步'}</button><button className="button primary" disabled={!!busy} onClick={onNext}>{busy === 'wizard' ? <LoaderCircle size={16} className="spin" /> : step === 2 ? <Check size={16} /> : null}{step === 2 ? '完成设置' : '下一步'}{step !== 2 && <ArrowRight size={15} />}</button></footer></div></div>;
}
