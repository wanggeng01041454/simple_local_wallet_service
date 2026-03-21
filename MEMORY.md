# MEMORY — 项目级智能体共享规则

本文件供所有 AI 智能体（Claude Code、Codex 等）遵循。

---

## 设计实现文档规则

### 1. 目录结构
- 设计文档 & 设计图：`docs/design/`
- 开发计划：`docs/plan/`

### 2. 命名规则

| 类型 | 命名模式 | 示例 |
|------|----------|------|
| 系统设计 | `System-Design-v{X}.{Y}.md` | `System-Design-v1.0.md` |
| 系统设计图 | `System-Design-v{X}.{Y}-seq.puml` / `.puml` / `.d2` | |
| 功能设计 | `Feature-{Name}-Design-v{X}.{Y}.md` | `Feature-User-Login-Design-v2.1.md` |
| 功能设计图 | `Feature-{Name}-Design-v{X}.{Y}-seq.puml` / `.puml` / `.d2` | |
| 开发计划 | `Plan-{Name}-v{X}.{Y}.md` | `Plan-User-Login-v2.1.md` |

- 功能名用 PascalCase + 连字符（如 `User-Login`）

### 3. 版本号规则
- **vX.0**：完整全新设计
- **vX.Y**（Y>0）：小改/补充/优化，兼容 vX.0
- **主版本号变（X 变）**：架构/逻辑大改，旧版本作废
- **Plan 版本号跟对应设计版本走**。设计不变但计划需修改时，直接原地修改计划文件，不创建新版本。

### 4. 索引文件
- `docs/INDEX.md` 是所有文档的总索引
- **每次新增、重命名或变更文档状态时，必须同步更新 `docs/INDEX.md`**

### 5. 文档固定头部（每个文档必须包含）

```
---
# 文档类型：系统设计 / 功能设计 / 开发计划
# 对应项目：xxx
# 版本：vX.Y
# 状态：初稿 / 已确认 / 已废弃
# 替代旧文档：xxx（无则写无）
# 被新文档替代：xxx（无则写无）
# 生效时间：YYYY-MM-DD
---
```
