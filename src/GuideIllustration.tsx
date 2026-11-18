import { useId } from 'react';

type GuideIllustrationProps = {
  kind: 'ps5' | 'obs';
};

// Inline SVG text shares the app's bundled font, including network addresses.
export function GuideIllustration({ kind }: GuideIllustrationProps) {
  const id = useId();
  const titleId = `${id}-title`;
  const descriptionId = `${id}-description`;

  if (kind === 'ps5') {
    return (
      <svg width="720" height="300" viewBox="0 0 720 300" role="img" aria-labelledby={`${titleId} ${descriptionId}`}>
        <title id={titleId}>PS5 手动 DNS 设置示意</title>
        <desc id={descriptionId}>DNS 设置选择手动，首选 DNS 填入 PlayStreamer 显示的电脑地址，备用 DNS 按界面允许留空或填 0.0.0.0。</desc>
        <rect width="720" height="300" rx="12" fill="#f3f6fc" />
        <text x="28" y="36" fontSize="17" fontWeight="600" fill="#23324b">PS5 · 网络高级设置</text>
        <text x="567" y="35" fontSize="11" fill="#728198">设置示意，非系统截图</text>
        <rect x="28" y="56" width="664" height="56" rx="6" fill="#fff" stroke="#dce3ee" />
        <text x="48" y="90" fontSize="14" fill="#34455e">DNS 设置</text>
        <text x="460" y="90" fontSize="14" fill="#175ce6">手动</text>
        <rect x="28" y="122" width="664" height="64" rx="6" fill="#e9f0ff" stroke="#175ce6" />
        <text x="48" y="160" fontSize="14" fontWeight="600" fill="#175ce6">首选 DNS</text>
        <rect x="435" y="136" width="235" height="36" rx="4" fill="#fff" stroke="#a4bff5" />
        <text x="452" y="159" fontSize="13" fill="#175ce6">粘贴首页显示的电脑地址</text>
        <rect x="28" y="196" width="664" height="56" rx="6" fill="#fff" stroke="#dce3ee" />
        <text x="48" y="230" fontSize="14" fill="#34455e">备用 DNS</text>
        <text x="435" y="230" fontSize="13" fill="#687990">留空或 0.0.0.0（如界面允许）</text>
        <text x="28" y="278" fontSize="12" fill="#687990">其他网络参数保留原值；不要把此处的电脑地址用作上游 DNS。</text>
      </svg>
    );
  }

  return (
    <svg width="720" height="330" viewBox="0 0 720 330" role="img" aria-labelledby={`${titleId} ${descriptionId}`}>
      <title id={titleId}>OBS 媒体源设置示意</title>
      <desc id={descriptionId}>添加媒体源，取消本地文件，将 PlayStreamer 提供的 RTMP 播放地址粘贴到输入字段，保留输入格式自动识别。</desc>
      <rect width="720" height="330" rx="12" fill="#f3f6fc" />
      <text x="28" y="36" fontSize="17" fontWeight="600" fill="#23324b">OBS · 媒体源「PlayStation」</text>
      <text x="567" y="35" fontSize="11" fill="#728198">设置示意，非软件截图</text>
      <rect x="28" y="56" width="664" height="234" rx="7" fill="#fff" stroke="#dce3ee" />
      <rect x="178" y="79" width="18" height="18" rx="3" fill="#fff" stroke="#90a1bc" />
      <text x="207" y="93" fontSize="14" fill="#34455e">本地文件</text>
      <text x="340" y="93" fontSize="12" fill="#175ce6">取消勾选，使用网络地址</text>
      <text x="72" y="147" fontSize="14" fill="#34455e">输入</text>
      <rect x="178" y="122" width="486" height="42" rx="4" fill="#e9f0ff" stroke="#175ce6" />
      <text x="191" y="148" fontSize="13" fill="#175ce6">rtmp://127.0.0.1:1935/live/playstation</text>
      <text x="72" y="200" fontSize="14" fill="#34455e">输入格式</text>
      <rect x="178" y="175" width="486" height="40" rx="4" fill="#f7f9fc" stroke="#dce3ee" />
      <text x="191" y="200" fontSize="13" fill="#8390a4">留空，自动识别</text>
      <rect x="572" y="237" width="92" height="34" rx="5" fill="#175ce6" />
      <text x="603" y="259" fontSize="13" fill="#fff">确定</text>
      <text x="28" y="315" fontSize="12" fill="#687990">此地址用于同一台电脑上的 OBS；主机实际开播后才会出现画面。</text>
    </svg>
  );
}
