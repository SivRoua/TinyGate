# 实时通知 v1

Web 和小程序通过 `wss://<部署域名>/api/v1/ws` 连接后端。实际主机由环境配置提供；本仓库不含部署地址。

## 鉴权与订阅

建立连接后 5 秒内发送：

```json
{"type":"authenticate","access_token":"<用户短期访问令牌>"}
```

服务端校验后回复 `{"type":"authenticated","schema_version":1}`。未鉴权不发送业务数据，失败或令牌过期用关闭码 1008 断开。令牌不得放在 URL 或日志中。

随后发送：

```json
{"type":"subscribe","gateway_ids":["gw_01"]}
```

服务端对每个网关鉴权；全部通过后回复 `{"type":"subscribed","gateway_ids":["gw_01"]}`，任一个失败则回复 `{"type":"error","code":"FORBIDDEN"}`，不改变原订阅。每次 subscribe 替换原订阅，上限 100 个网关。控制帧也由 messages.schema.json 的 ClientFrame / ServerControlFrame 定义。

## 数据通知

RealtimeNotification 的 type 为 device.state_changed、gateway.status_changed 或 command.updated。data 分别为完整 Device、Gateway 或 Command 快照；网关与设备快照自带 state_revision，命令快照自带 command_revision。前端只应用更高修订号，同号忽略。

订阅确认后再发 HTTP 快照请求；等待期间可缓存通知并按修订号合并，避免“查快照后订阅”遗漏变化。通知示例见 examples/scenarios.json。

## 断线恢复

- 本版不提供 WebSocket 历史回放；连接恢复后重新鉴权、订阅并查询 HTTP 快照和未完成命令。
- 使用应用层 `{"type":"ping"}` / `{"type":"pong"}`，客户端每 20 秒发送，45 秒无响应重连；指数退避建议 1、2、4、8 秒，上限 30 秒并带随机抖动。
- WebSocket 不可用时每 2 秒通过 GET commands 查询未完成命令，终态停止轮询；设备列表可每 10 秒刷新。
- WSS 只发送通知，创建控制始终走 POST commands，从而统一鉴权、幂等和审计。
