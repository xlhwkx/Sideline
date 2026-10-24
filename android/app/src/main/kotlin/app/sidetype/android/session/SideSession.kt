package app.sidetype.android.session

import app.sidetype.android.protocol.Auth
import app.sidetype.android.protocol.ClientMessage
import app.sidetype.android.protocol.DesktopWindow
import app.sidetype.android.protocol.Focus
import app.sidetype.android.protocol.KeyPress
import app.sidetype.android.protocol.Maximize
import app.sidetype.android.protocol.Minimize
import app.sidetype.android.protocol.PairRequest
import app.sidetype.android.protocol.Ping
import app.sidetype.android.protocol.Pong
import app.sidetype.android.protocol.ServerMessage
import app.sidetype.android.protocol.Text
import app.sidetype.android.protocol.WindowsRequest
import app.sidetype.android.protocol.describe
import app.sidetype.android.protocol.parse
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import java.util.UUID
import java.util.concurrent.Executors
import java.util.concurrent.ScheduledFuture
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean

/** 界面只观察这个状态。 */
sealed interface SessionState {
    data object Disconnected : SessionState
    data object Connecting : SessionState
    data class Connected(val name: String) : SessionState
    data class Failed(val message: String) : SessionState
}

/** 一条到电脑的连接。断线后按退避重连，不持有任何界面。 */
class SideSession(
    private val store: TokenStore,
    private val onState: (SessionState) -> Unit,
    private val onWindows: (List<DesktopWindow>) -> Unit = {},
) {
    private val client = OkHttpClient.Builder()
        .pingInterval(5, TimeUnit.SECONDS)
        .build()
    private val stopped = AtomicBoolean(false)
    private val scheduler = Executors.newSingleThreadScheduledExecutor()
    private var socket: WebSocket? = null
    private var retry: ScheduledFuture<*>? = null
    private var delayMs = 1_000L
    private var endpoint: Pair<String, Int>? = null

    /** 用记住的 token 重连。没有记住的电脑时什么都不做。 */
    fun reconnect() {
        val saved = store.load() ?: return
        connect(saved.host, saved.port, Auth(saved.token))
    }

    /** 用配对码连接一台新电脑。 */
    fun pair(host: String, port: Int, code: String) {
        connect(host, port, PairRequest(code))
    }

    /** 发送正文。 */
    fun sendText(text: String, mode: String, enter: Boolean) {
        send(Text(UUID.randomUUID().toString(), text, mode, enter))
    }

    /** 要一份电脑上可以切换的窗口。结果从构造时的回调回来。 */
    fun requestWindows() {
        send(WindowsRequest)
    }

    /** 把一个窗口拉到前台。 */
    fun focus(id: String) {
        send(Focus(id))
    }

    /** 最小化一个窗口。 */
    fun minimize(id: String) {
        send(Minimize(id))
    }

    /** 最大化一个窗口。已经最大化时还原。 */
    fun maximize(id: String) {
        send(Maximize(id))
    }

    /** 发送一个单独按键。 */
    fun sendKey(key: String) {
        send(KeyPress(UUID.randomUUID().toString(), key))
    }

    /** 忘掉这台电脑并断开。 */
    fun forget() {
        stopped.set(true)
        retry?.cancel(false)
        store.clear()
        endpoint = null
        socket?.close(1000, "forget")
        onState(SessionState.Disconnected)
    }

    /** 进程退出时断开，不再重连。 */
    fun stop() {
        stopped.set(true)
        retry?.cancel(false)
        scheduler.shutdownNow()
        socket?.close(1000, "stop")
    }

    private fun connect(host: String, port: Int, hello: ClientMessage) {
        stopped.set(false)
        retry?.cancel(false)
        delayMs = 1_000L
        endpoint = host to port
        socket?.cancel()
        onState(SessionState.Connecting)
        val request = Request.Builder().url("ws://$host:$port/v1").build()
        socket = client.newWebSocket(request, object : WebSocketListener() {
            override fun onOpen(webSocket: WebSocket, response: Response) {
                webSocket.send(hello.toJson())
                webSocket.send(Ping(System.currentTimeMillis()).toJson())
            }

            override fun onMessage(webSocket: WebSocket, text: String) {
                when (val message = parse(text)) {
                    is ServerMessage.PairOk -> {
                        store.save(SavedComputer(host, port, "", message.token))
                        onState(SessionState.Connected(""))
                    }
                    is ServerMessage.AuthOk -> onState(SessionState.Connected(message.name))
                    is ServerMessage.Ack -> if (!message.ok) onState(SessionState.Failed(describe(message.error)))
                    is ServerMessage.Failure -> onState(SessionState.Failed(describe(message.error)))
                    is ServerMessage.Ping -> webSocket.send(Pong(message.t).toJson())
                    is ServerMessage.Windows -> onWindows(message.windows)
                    is ServerMessage.Kick -> {
                        store.clear()
                        onState(SessionState.Failed("电脑已取消配对"))
                    }
                    ServerMessage.Pong, ServerMessage.Unknown -> Unit
                }
            }

            override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
                if (webSocket !== socket) return
                drop("没有连上")
            }

            override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
                if (webSocket !== socket) return
                drop("没有连上")
            }
        })
    }

    /** 连接掉了就按退避重连。用户主动断开或电脑取消配对时不重连。 */
    private fun drop(message: String) {
        if (stopped.get()) return
        onState(SessionState.Failed(message))
        val saved = store.load() ?: return
        val wait = delayMs
        delayMs = (delayMs * 2).coerceAtMost(15_000L)
        retry = scheduler.schedule({
            if (!stopped.get()) connect(saved.host, saved.port, Auth(saved.token))
        }, wait, TimeUnit.MILLISECONDS)
    }

    private fun send(message: ClientMessage) {
        socket?.send(message.toJson())
    }
}
