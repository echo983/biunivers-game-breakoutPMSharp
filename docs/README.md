# 文档索引与规范

> 本目录为 Biunivers Breakout 的项目文档。命名、职责与修改规则如下。

## 文档清单

| 文件 | 角色 | 修改规则 |
|---|---|---|
| `README.md`（仓库根） | 用户入口：玩法、架构、构建、目录 | 随功能更新 |
| `docs/design-v0.1.md` | 设计草案 v0.1（《物件球拍物理破坏游戏》总纲） | 可演进；标定参数追加「已标定」注释 |
| `docs/设计草案1.txt` | 设计草案原始文本（v0.1 的原始来源） | 只读参考；如要改，以整理版为准 |
| `docs/design-v1.md` | v1 完整版本设计（首个可玩版本规格） | 可演进；评审修复记入附录 |
| `docs/construction-plan-v1.1.md` | v1.1 施工计划（范围/方案/验收/里程碑） | 施工阶段依据；随实现更新 |
| `docs/playtest-checklist.md` | v1 实测清单（手感数据 + 参数微调建议） | 每次版本迭代复用；实测结果记入 §6 |
| `docs/lessons-learned.md` | 开发日志：重大错误、排查方法与经验 | 追加式维护，不删旧条目 |
| `CHANGELOG.md`（仓库根） | 发布历史（SemVer 版本 → 变更） | 每次行为变更/发布新增条目 |
| `docs/README.md` | 本文档 | — |

## 冻结（请勿修改）文件

以下为 Biunivers 协议原文，安装校验用，**不得**改写/翻译/摘要/修复/重生成：

- `BIUNIVERS_APP_PROTOCOL_V1.md`
- `BIUNIVERS_OPEN_RESOURCE_PROTOCOL_V1.md`（如存在）
- `BIUNIVERS_OPEN_RESOURCE_PROTOCOL_V1_1.md`（如存在）
- `BIUNIVERS_RESOURCE_SESSION_PROTOCOL_V1.md`（如存在）

`BIUNIVERS_GAME_RUNTIME_PROTOCOL_V1.md` / `_V2.md` 是本项目的**草稿**（非冻结），可在显式
冻结前随项目演进；v1 当前外壳接口（`render_backend/.../destroy`）与实现保持一致。

## 命名与版本规范

- **文档命名**：正式稿用 `design-<版本>.md`（如 `design-v1.md`）；原始/中间稿可保留来源
  文件名，但须在索引中标注角色。
- **应用版本**：`biunivers.app.json` 的 `version` 采用 SemVer；行为/品牌变更必须递增，
  并在 `CHANGELOG.md` 登记。分支开发期可连续递增（0.5.0 → 0.5.1 → …），合并发布时以
  分支末态为准。
- **协议版本**：外壳接口变化需演进 `BIUNIVERS_GAME_RUNTIME_PROTOCOL_V2.md` 并在 README
  同步接口清单。

## 施工阶段约定

- 施工前：完成设计 → 规格（design-v1 或更新版）→ 评审自洽性/可行性 → 记录遗留项。
- 施工中：无。阶段明确为「文档/规范性」时不做代码改动。
- 施工后：更新 README/CHANGELOG/lessons-learned，按 AGENTS.md 完成报告清单（冻结协议
  逐字节核对、无 secret、manifest 校验、最小窗口可用）。

## 校验清单（每次变更后）

1. `biunivers.app.json` 为合法 JSON 且 `version` 递增合理。
2. 冻结协议文件未被触碰（`git log -- <file>` 只应含初始化提交）。
3. 无新增 secret/凭据（浅扫 `password/secret/api_key/token/private_key`）。
4. 游戏外壳接口清单与 v2 协议一致。
5. 最小/默认窗口可用性在渲染与布局层保持（砖块列数、相机取景）。