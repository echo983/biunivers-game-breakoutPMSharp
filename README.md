# Biunivers Breakout

经典打砖块（Breakout）游戏，作为 Biunivers 静态桌面应用运行。

- 协议：`biunivers.static-app/1`
- 入口：`index.html`（仓库根目录）
- 无构建、无依赖、无后端、无 secret

## 本地运行

任意静态文件服务器均可，例如：

```bash
python3 -m http.server 8000
```

然后访问 <http://localhost:8000/>。

## 目录

- `index.html`：应用入口
- `app.js`：游戏逻辑
- `style.css`：样式
- `icon.svg`：桌面图标
- `biunivers.app.json`：应用清单
- `BIUNIVERS_APP_PROTOCOL_V1.md`：协议原文（请勿修改）
- `AGENTS.md`：AI 开发代理约束

## 配置

暂无公开配置项。