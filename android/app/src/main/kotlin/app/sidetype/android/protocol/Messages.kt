package app.sidetype.android.protocol

import org.json.JSONObject

/** 协议版本，和 protocol/v1.md 一致。 */
const val VERSION = 1

/** 单条正文的字节上限。 */
const val TEXT_LIMIT = 100_000

/** 手机发给电脑的消息。 */
sealed interface ClientMessage {
    fun toJson(): String
}

data class PairRequest(val code: String) : ClientMessage {
    override fun toJson() = json("pair", "code" to code)
}

data class Auth(val token: String) : ClientMessage {
    override fun toJson() = json("auth", "token" to token)
}

data class Text(
    val id: String,
    val text: String,
    val mode: String,
    val enter: Boolean,
) : ClientMessage {
    override fun toJson() = json("text", "id" to id, "text" to text, "mode" to mode, "enter" to enter)
}

data class KeyPress(val id: String, val key: String) : ClientMessage {
    override fun toJson() = json("key", "id" to id, "key" to key)
}

data class Ping(val t: Long) : ClientMessage {
    override fun toJson() = json("ping", "t" to t)
}

data class Pong(val t: Long) : ClientMessage {
    override fun toJson() = json("pong", "t" to t)
}

data object WindowsRequest : ClientMessage {
    override fun toJson() = json("windows")
}

data class Focus(val id: String) : ClientMessage {
    override fun toJson() = json("focus", "id" to id)
}

data class Minimize(val id: String) : ClientMessage {
    override fun toJson() = json("minimize", "id" to id)
}

data class Maximize(val id: String) : ClientMessage {
    override fun toJson() = json("maximize", "id" to id)
}

/** 电脑发来的消息。解析不了就是 [Unknown]。 */
sealed interface ServerMessage {
    data class PairOk(val token: String) : ServerMessage
    data class AuthOk(val name: String) : ServerMessage
    data class Ack(val id: String, val ok: Boolean, val error: String?) : ServerMessage
    data class Kick(val reason: String) : ServerMessage
    data class Ping(val t: Long) : ServerMessage
    data class Failure(val error: String) : ServerMessage
    data class Windows(val windows: List<DesktopWindow>) : ServerMessage
    data object Pong : ServerMessage
    data object Unknown : ServerMessage
}

/** 电脑上一个可以切换过去的窗口。 */
data class DesktopWindow(
    val id: String,
    val title: String,
    val topmost: Boolean,
    val minimized: Boolean,
    val maximized: Boolean,
    val icon: String,
)

/** 把一帧 JSON 解析成消息。损坏的 JSON 也是 [ServerMessage.Unknown]。 */
fun parse(frame: String): ServerMessage {
    val json = runCatching { JSONObject(frame) }.getOrNull() ?: return ServerMessage.Unknown
    return when (json.optString("type")) {
        "pair_ok" -> ServerMessage.PairOk(json.optString("token"))
        "auth_ok" -> ServerMessage.AuthOk(json.optString("name"))
        "ack" -> ServerMessage.Ack(json.optString("id"), json.optBoolean("ok"), json.optString("error").ifEmpty { null })
        "kick" -> ServerMessage.Kick(json.optString("reason"))
        "ping" -> ServerMessage.Ping(json.optLong("t"))
        "error" -> ServerMessage.Failure(json.optString("error"))
        "windows" -> ServerMessage.Windows(
            json.optJSONArray("windows")?.let { array ->
                (0 until array.length()).map { index ->
                    val item = array.getJSONObject(index)
                    DesktopWindow(
                        item.optString("id"),
                        item.optString("title"),
                        item.optBoolean("topmost"),
                        item.optBoolean("minimized"),
                        item.optBoolean("maximized"),
                        item.optString("icon"),
                    )
                }
            }.orEmpty(),
        )
        "pong" -> ServerMessage.Pong
        else -> ServerMessage.Unknown
    }
}

/** 把错误码翻成一行中文。 */
fun describe(error: String?): String = when (error) {
    "bad_code" -> "配对码不正确或已过期"
    "bad_token" -> "这台电脑已取消配对，请重新配对"
    "unauthorized" -> "还没有连接"
    "busy" -> "上一句还在发送，请稍等"
    "integrity" -> "目标窗口以管理员运行，请同样以管理员运行电脑上的 Sideline"
    "empty" -> "没有内容"
    "too_large" -> "内容太长"
    "inject_failed" -> "没有送进去，请确认电脑上有一个正在输入的窗口"
    "clipboard_race" -> "已发送，但剪贴板被其他程序改过，没有还原"
    else -> "没有送进去"
}

private fun json(type: String, vararg fields: kotlin.Pair<String, Any>): String {
    val json = JSONObject()
    json.put("v", VERSION)
    json.put("type", type)
    fields.forEach { (key, value) -> json.put(key, value) }
    return json.toString()
}
