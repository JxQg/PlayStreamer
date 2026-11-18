export type GuideId = 'ps5' | 'obs' | 'bilibili' | 'oopz' | 'troubleshooting';
export interface GuideSection { title: string; paragraphs: string[]; note?: string }
export interface Guide { id: GuideId; title: string; subtitle: string; time: string; sections: GuideSection[]; links?: { label: string; url: string }[] }

export const guides: Guide[] = [
  {
    id: 'ps5', title: '连接 PlayStation 5', subtitle: '在同一局域网内，把主机的直播画面交给电脑。', time: '首次设置',
    sections: [
      { title: '准备主机和电脑', paragraphs: ['将 PS5 和电脑连接至同一路由器，优先使用有线网络。访客 Wi-Fi、AP 隔离或不同 VLAN 可能阻止主机访问电脑。', '在 PS5「设置 → 网络 → 连接状态 → 查看连接状态」记录主机 IPv4 地址，填入 PlayStreamer 的连接设置。选择与主机相连的物理以太网或 Wi-Fi 网卡，不要选择 TUN、VPN 或虚拟机网卡。建议在路由器中为电脑和 PS5 保留固定 DHCP 地址。'] },
      { title: '关联 Twitch 账号', paragraphs: ['在 PS5 的「设置 → 用户和账号 → 已关联的服务」中关联 Twitch，并按 Twitch 提示完成账号验证。界面名称可能随系统版本变化。', '关联账号和启动直播仍需要主机能够访问 PSN 与 Twitch 的相关服务；本地接收不会代替账号登录或解决这些服务的网络可达性。'] },
      { title: '保留已有 TUN 或代理', paragraphs: ['PlayStreamer 不会关闭 TUN 或修改系统代理。接流使用物理局域网，按代理产品的文档让回环、电脑局域网地址、PS5 地址和对应网段直连，并检查 DNS 劫持或 53 端口冲突。', '电脑能访问 Twitch 不代表 PS5 也能访问；仅在电脑开启 TUN 通常不会自动转发主机流量。可以保留路由器上已有的联网方案。', '如复用电脑代理，需由该产品提供允许 PS5 访问的 HTTP / mixed 局域网监听，再在 PS5 配置电脑物理 IP 和代理端口。仅 SOCKS 端口不可直接使用，也不能保证 HTTP 代理覆盖所有主机协议。'], note: '应用的上游 DNS 只负责普通域名解析，不是代理地址。不要直接照抄 TUN 虚拟 DNS 或 fake-IP 配置；直播入口必须解析到电脑真实局域网地址。' },
      { title: '设置 PS5 的 DNS', paragraphs: ['在 PlayStreamer 选择与 PS5 相连的网卡，保存后开启接收。复制首页显示的「PS5 首选 DNS」地址。', '进入 PS5「设置 → 网络 → 设置 → 设置互联网连接」，选择当前网络的「高级设置」，将 DNS 设置改为手动，首选 DNS 填入上述电脑地址。其他网络参数保留原有值。', '备用 DNS 按主机界面允许留空或填 0.0.0.0。不要填公共 DNS，否则 PS5 可能绕过本地接收。PlayStreamer 的上游 DNS 负责正常域名解析。'], note: 'PS5 使用电脑作为 DNS 后，电脑需保持开机且 PlayStreamer DNS 服务运行。完全退出前，请将 PS5 的 DNS 恢复为自动。' },
      { title: '开始直播', paragraphs: ['按手柄 Create 键，选择「播送 → Twitch」，设置标题与画质并开始播送。首次建议从 720p 开始，确认稳定后再提高画质。', '首页出现「正在接收」表示电脑已实际收到直播流；「等待主机开播」只表示接收服务已启动。若始终未收到，请打开诊断查看 DNS 命中与媒体服务状态。'], note: '此方式依赖 PS5 当前采用的直播协议和域名。若系统更新改用无法兼容的加密链路，单纯修改 DNS 无法解决；请以实际接收状态为准。' },
      { title: '希望不使用 Twitch', paragraphs: ['当前模式仍需 Twitch 关联和开播授权。DNS 重定向只改变媒体目的地；把登录域名指向电脑不能代替 HTTPS 和真实平台授权。目前没有已验证的免账号原生广播方案。', '可以改用官方 PS Remote Play 或 chiaki-ng 远程游玩窗口交给 OBS，完全避开 Twitch。官方客户端仍需 PlayStation 账号；chiaki-ng 本地手动注册可使用 Account-ID 和主机配对 PIN，仍需设备认证。', 'HDMI 采集卡也能直接接入 OBS，无需 Twitch，但需要额外硬件并遵守受保护内容限制。这些方式是其他画面来源，本版本未内置远程游玩或采集卡后端。'] },
    ],
    links: [{ label: 'PlayStation 官方播送帮助', url: 'https://www.playstation.com/support/games/ps5-stream-gameplay-broadcast/' }, { label: '官方 PC / Mac 远程游玩', url: 'https://www.playstation.com/en-us/support/games/playstation-remote-play-on-pc-and-mac/' }],
  },
  {
    id: 'obs', title: '将画面接入 OBS', subtitle: '用 OBS 完成场景、麦克风、混音和直播输出。', time: '约 2 分钟',
    sections: [
      { title: '安装并打开 OBS Studio', paragraphs: ['从 OBS 官网安装 Windows 版 OBS Studio。PlayStreamer 负责接收主机直播，OBS 需要单独安装并运行。', '先在 PlayStreamer 开启接收，再从 PS5 发起直播。确认首页已显示「正在接收」。'] },
      { title: '添加媒体源', paragraphs: ['在 OBS 的「来源」区域点击「＋ → 媒体源」，命名为「PlayStation」。取消勾选「本地文件」，把 PlayStreamer 首页复制的 OBS 播放地址粘贴至「输入」。', '地址中的 127.0.0.1 代表当前电脑，因此此地址用于同一台电脑上的 OBS。保留自动输入格式，确认后等待画面出现。', '如需切换场景后继续接收，可取消「源不可见时关闭文件」。缓冲和硬件解码选项先保留默认值，再根据实际稳定性调整。'] },
      { title: '检查声音与场景', paragraphs: ['确认「音频混音器」中的 PlayStation 媒体源有电平。需要自己的解说时，添加麦克风输入，并避免把同一声音经桌面音频重复采集。', '建议先录制十几秒回看，确认画面、音量和音画同步。主机上受保护内容、部分游戏场景或聊天音频可能不会出现在播送中。'] },
      { title: '选择输出平台', paragraphs: ['Bilibili 使用直播中心提供的服务器和串流密钥进行 OBS 推流。OOPZ 优先按其当前客户端支持的屏幕或窗口共享方式发送画面。', 'PlayStreamer 显示的 RTMP 地址是 OBS 读取画面的地址，不能填入平台的串流密钥栏，也不是让 OBS 向此处推流。'] },
    ],
    links: [{ label: '下载 OBS Studio', url: 'https://obsproject.com/download' }, { label: 'OBS 官方帮助', url: 'https://obsproject.com/help' }],
  },
  {
    id: 'bilibili', title: '转播至 Bilibili', subtitle: '让 OBS 将完成混音的直播场景发送到 B 站。', time: '平台配置',
    sections: [
      { title: '获取推流信息', paragraphs: ['登录 Bilibili 直播中心，完成平台要求的实名认证和开播资格。进入开播设置，按当前页面提示获取推流地址和串流密钥。', '不同账号、分区或开播方式的权限可能不同。若页面没有提供第三方推流信息，应按平台官方指引申请或使用其支持的开播方式。'] },
      { title: '在 OBS 配置直播服务', paragraphs: ['打开 OBS「设置 → 直播」，选择 Bilibili 服务（如果当前版本提供），或选择「自定义」。', '把平台提供的推流地址填入「服务器」，把串流密钥填入「串流密钥」。按平台要求设置视频分辨率、帧率与码率，不直接照搬 PS5 的输入参数。'], note: '串流密钥可以用于向你的直播间推流。只在 OBS 内填写，不要发到聊天、日志或截图中。PlayStreamer 不需要保存它。' },
      { title: '开播并确认', paragraphs: ['先确认 OBS 的 PlayStation 媒体源有画面和声音，再点击「开始直播」。部分直播间还需要在网页上确认开播或选择分区。', '用另一台设备查看直播间。OBS 显示的丢帧和输出码率用于判断到平台的链路，PlayStreamer 的接收码率则反映 PS5 到电脑的输入链路。'] },
    ],
    links: [{ label: '打开 Bilibili 直播中心', url: 'https://link.bilibili.com/p/center/index' }],
  },
  {
    id: 'oopz', title: '在 OOPZ 中分享', subtitle: '通过客户端支持的分享方式，把 OBS 场景带入频道。', time: '平台配置',
    sections: [
      { title: '准备 OBS 画面', paragraphs: ['先按照 OBS 教程接入 PlayStation，确认画面和声音正常。右键 OBS 预览区域，打开「窗口化投影（预览）」或当前版本提供的相应投影窗口。', '投影窗口只展示直播场景，可避免在分享画面中暴露 OBS 的设置面板。'] },
      { title: '在 OOPZ 选择分享窗口', paragraphs: ['进入 OOPZ 的目标语音频道，使用当前客户端提供的屏幕或窗口分享功能，选择 OBS 投影窗口。按钮名称和可用能力以当前 OOPZ 版本为准。', '如果 OOPZ 的相应入口支持摄像头，可以尝试 OBS 虚拟摄像头；虚拟摄像头仅提供视频，不能把它当作同时传递音频的方案。'], note: '本版本不假设 OOPZ 提供公开 RTMP 推流入口，也不会生成虚构的 OOPZ 服务器地址。' },
      { title: '确认声音', paragraphs: ['若 OOPZ 提供「分享窗口声音」或系统音频选项，按客户端说明启用，并请频道中的另一位成员确认能听到游戏音频。', '如果当前分享方式不支持音频，需要使用 OOPZ 支持的音频采集方式另行配置。不要同时监听并采集同一声音，避免回声。'] },
    ],
    links: [{ label: '打开 OOPZ 官网', url: 'https://oopz.cn/' }],
  },
  {
    id: 'troubleshooting', title: '连接与故障排查', subtitle: '沿着主机、接收、OBS、平台逐段检查。', time: '遇到问题时',
    sections: [
      { title: '主机不能联网或无法开始播送', paragraphs: ['确认 PlayStreamer 的 DNS 服务运行、PS5 首选 DNS 是当前电脑地址、PS5 IP 与设置一致。电脑换网或 DHCP 地址变化后，需要更新对应设置。', '检查电脑与 PS5 是否处于同一局域网，路由器是否开启访客隔离。完全退出 PlayStreamer 后，先将主机 DNS 恢复自动，再检查常规网络。', 'PSN / Twitch 登录失败需单独确认其可达性和账号状态；本地拦截并不会代理这些服务。'] },
      { title: '开启 TUN 后无法连接', paragraphs: ['先确认接流选中物理网卡，检查代理产品对回环、电脑局域网地址、PS5 地址及该网段的直连规则。TUN 保持开启也可以使用本地接流，但默认路由不能单独证明是否存在流量接管。', '检查 TCP/UDP 53 是否被其他 DNS 监听占用，代理 DNS 劫持是否影响主机请求，直播域名是否被替换成 fake-IP。按代理产品文档调整冲突项，不必直接关闭整个代理或防火墙。', '上游 DNS 与代理入口分开配置。若电脑可访问 Twitch 但主机不能，应在 PS5 单独验证已有路由器或 HTTP 代理方案；PlayStreamer 不提供主机流量转接。'] },
      { title: '一直显示等待开播', paragraphs: ['在诊断中查看 DNS 查询与拦截是否发生。无 DNS 查询通常说明主机没有使用本机 DNS，或防火墙、网卡选择、IP 限制不匹配。', '有 DNS 命中但没有媒体流时，确认主机实际已发起 Twitch 播送，并检查媒体服务是否启动、TCP 1935 是否被占用。主机协议变化也可能造成不兼容。', '可使用设置中的「配置 Windows 防火墙」为本应用配置必要规则；系统可能要求管理员确认。不要为排障直接关闭整个防火墙。'] },
      { title: 'OBS 黑屏或播放中断', paragraphs: ['只有 PlayStreamer 显示「正在接收」时才存在可播放的直播画面。确认 OBS 使用媒体源、取消本地文件，并输入首页给出的完整地址。', '如果媒体源已创建，可重新打开其属性并确认，或重新激活来源。检查 TCP 1935 冲突、电脑睡眠与主机结束播送。', '网络变更或服务异常时，先查看明确错误信息，再停止并重新开启接收。不要反复修改随机端口。'] },
      { title: '卡顿、延迟或声音重复', paragraphs: ['优先将电脑与 PS5 接入有线网络。逐步降低主机直播画质，观察输入码率和 OBS 的统计窗口，区分接收问题与平台输出问题。', '避免在 OBS 同时采集媒体源音频、桌面回放与麦克风回声。此链路经过主机编码及媒体缓冲，适合直播观看，不建议根据 OBS 预览操控游戏。'] },
      { title: '导出诊断信息', paragraphs: ['在「设置与诊断」运行检查，再导出诊断文件。文件路径会显示在客户端中，可在反馈时按需提供。', '分享前检查是否含有你不希望公开的局域网地址。请不要另外附带 Twitch 或直播平台的串流密钥。'] },
    ],
  },
];
