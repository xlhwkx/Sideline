package app.sidetype.android.ui

import android.net.Uri
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.rememberScrollState
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.outlined.Close
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material.icons.outlined.WebAsset
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.PathBuilder
import androidx.compose.ui.graphics.vector.path
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SuggestionChip
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.key
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import kotlinx.coroutines.delay
import androidx.compose.ui.Alignment
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import app.sidetype.android.protocol.DesktopWindow
import app.sidetype.android.session.SessionState
import app.sidetype.android.ui.theme.MagpieStatus

/** 扫码得到的一台电脑。`code` 为空说明是旧版二维码，需要手填。 */
data class ScannedAddress(val host: String, val port: Int, val code: String)

/** 解析 `sidetype://pair?host=&port=&code=`。格式不对时返回 null。 */
fun parsePairUri(raw: String): ScannedAddress? {
    val uri = runCatching { Uri.parse(raw) }.getOrNull() ?: return null
    if (uri.scheme != "sidetype") return null
    val host = uri.getQueryParameter("host") ?: return null
    if (host.isBlank()) return null
    val port = uri.getQueryParameter("port")?.toIntOrNull() ?: 17331
    val code = uri.getQueryParameter("code").orEmpty()
    return ScannedAddress(host, port, code)
}

/** 整个界面。状态从外面传进来，自己不发网络请求。 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SideTypeApp(
    state: SessionState,
    onScan: () -> Unit,
    onPair: (host: String, port: Int, code: String) -> Unit,
    onSend: (text: String, mode: String, enter: Boolean) -> Unit,
    onKey: (key: String) -> Unit,
    windows: List<DesktopWindow>,
    onRefreshWindows: () -> Unit,
    onFocus: (String) -> Unit,
    onMinimize: (String) -> Unit,
    onMaximize: (String) -> Unit,
    onForget: () -> Unit,
) {
    val status = MagpieStatus(dark = isSystemInDarkTheme())
    val typing = state is SessionState.Connected || state is SessionState.Failed && state.message.startsWith("已发送")
    var settings by rememberSaveable { mutableStateOf(false) }
    var windowsOpen by rememberSaveable { mutableStateOf(false) }
    var mode by rememberSaveable { mutableStateOf("paste") }
    Scaffold(containerColor = MaterialTheme.colorScheme.background) { insets ->
        Column(modifier = Modifier.fillMaxSize().statusBarsPadding().padding(insets).imePadding()) {
            when {
                windowsOpen -> WindowScreen(windows, onRefreshWindows, onFocus, onMinimize, onMaximize, { windowsOpen = false })
                settings -> SettingsScreen(mode, { mode = it }, onForget, { settings = false })
                typing -> TypeScreen(state, status, mode, onSend, onKey, { settings = true }, {
                    windowsOpen = true
                    onRefreshWindows()
                })
                else -> PairScreen(state, status, onScan, onPair)
            }
        }
    }
}

@Composable
private fun PairScreen(
    state: SessionState,
    status: MagpieStatus,
    onScan: () -> Unit,
    onPair: (host: String, port: Int, code: String) -> Unit,
) {
    var manual by rememberSaveable { mutableStateOf(false) }
    var host by rememberSaveable { mutableStateOf("") }
    var code by rememberSaveable { mutableStateOf("") }
    val ready = host.isNotBlank() && code.length == 6
    Column(modifier = Modifier.fillMaxSize().padding(horizontal = 24.dp)) {
        Spacer(Modifier.height(48.dp))
        Text("Sideline", style = MaterialTheme.typography.headlineLarge)
        Spacer(Modifier.height(8.dp))
        Text(
            "用手机打字\n文字出现在电脑上",
            style = MaterialTheme.typography.titleLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(20.dp))
        StatusChip(state, status)
        Spacer(Modifier.weight(1f))
        Button(onClick = onScan, modifier = Modifier.fillMaxWidth().height(56.dp), shape = MaterialTheme.shapes.extraLarge) {
            Text("扫描二维码", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
        }
        TextButton(onClick = { manual = !manual }, modifier = Modifier.align(Alignment.CenterHorizontally).padding(top = 4.dp, bottom = 12.dp)) {
            Text(if (manual) "收起" else "手动连接")
        }
        if (manual) {
            Card(
                shape = MaterialTheme.shapes.large,
                colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant),
            ) {
                Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    OutlinedTextField(
                        value = host,
                        onValueChange = { host = it.trim() },
                        modifier = Modifier.fillMaxWidth(),
                        label = { Text("电脑地址") },
                        singleLine = true,
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
                        shape = MaterialTheme.shapes.small,
                    )
                    OutlinedTextField(
                        value = code,
                        onValueChange = { if (it.length <= 6) code = it.filter(Char::isDigit) },
                        modifier = Modifier.fillMaxWidth(),
                        label = { Text("配对码") },
                        singleLine = true,
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                        shape = MaterialTheme.shapes.small,
                    )
                    FilledTonalButton(
                        onClick = { val (h, p) = splitHost(host); onPair(h, p, code) },
                        enabled = ready,
                        modifier = Modifier.fillMaxWidth().height(48.dp),
                        shape = MaterialTheme.shapes.large,
                    ) { Text("连接") }
                }
            }
            Spacer(Modifier.height(16.dp))
        }
    }
}

@Composable
private fun TypeScreen(
    state: SessionState,
    status: MagpieStatus,
    mode: String,
    onSend: (text: String, mode: String, enter: Boolean) -> Unit,
    onKey: (key: String) -> Unit,
    onSettings: () -> Unit,
    onWindows: () -> Unit,
) {
    var text by rememberSaveable { mutableStateOf("") }
    var sent by rememberSaveable { mutableStateOf("") }
    var field by rememberSaveable { mutableIntStateOf(0) }
    val send = rememberUpdatedState(onSend)
    val key = rememberUpdatedState(onKey)
    LaunchedEffect(text, mode) {
        if (mode == "type" || text.isEmpty() || !committed(text)) return@LaunchedEffect
        delay(SETTLE_MS)
        val edit = tailEdit(sent, text) ?: return@LaunchedEffect
        repeat(edit.erase) { key.value("backspace") }
        if (edit.add.isNotEmpty()) send.value(edit.add, "paste", false)
        sent = text
    }
    Column(modifier = Modifier.fillMaxSize().padding(horizontal = 20.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 12.dp)) {
            Text("Sideline", style = MaterialTheme.typography.headlineLarge)
            Spacer(Modifier.weight(1f))
            IconButton(onClick = onWindows) {
                Icon(Icons.Outlined.WebAsset, contentDescription = "窗口", tint = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            IconButton(onClick = onSettings) {
                Icon(Icons.Outlined.Settings, contentDescription = "设置", tint = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
        StatusChip(state, status)
        Spacer(Modifier.height(20.dp))
        Box(modifier = Modifier.fillMaxWidth().weight(1f)) {
        key(field) {
        OutlinedTextField(
            value = text,
            onValueChange = { raw ->
                val next = raw.replace(Regex("aN;|NaNM"), "")
                if (mode == "paste" && sent.isNotEmpty() && sent.startsWith(next) && next.length < sent.length) {
                    repeat(sent.length - next.length) { key.value("backspace") }
                    sent = next
                } else {
                    forwardDiff(text, next, mode, send.value, key.value)
                }
                text = next
            },
            modifier = Modifier.fillMaxSize().heightIn(min = 160.dp),
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Send),
            keyboardActions = KeyboardActions(onSend = {
                key.value("enter")
                sent = ""
                text = ""
            }),
            placeholder = { Text("打字，电脑上同步出现", color = MaterialTheme.colorScheme.onSurfaceVariant) },
            shape = MaterialTheme.shapes.extraLarge,
            textStyle = MaterialTheme.typography.headlineMedium,
            colors = androidx.compose.material3.OutlinedTextFieldDefaults.colors(
                focusedContainerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
                unfocusedContainerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
                focusedBorderColor = androidx.compose.ui.graphics.Color.Transparent,
                unfocusedBorderColor = androidx.compose.ui.graphics.Color.Transparent,
                cursorColor = MaterialTheme.colorScheme.primary,
            ),
        )
        }
        if (text.isNotEmpty() || sent.isNotEmpty()) {
            IconButton(
                onClick = {
                    val removing = text.length
                    repeat(removing) { key.value("backspace") }
                    sent = ""
                    text = ""
                    field += 1
                },
                modifier = Modifier.align(Alignment.BottomEnd).padding(8.dp),
            ) {
                Icon(Icons.Outlined.Close, contentDescription = "清空", tint = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
        }
        ShortcutRow(onKey)
        Spacer(Modifier.height(16.dp))
    }
}

/** 发到电脑当前窗口的组合键。显示键位，读屏读动作。 */
private data class Shortcut(val name: String, val label: String, val description: String)

private val SHORTCUTS = listOf(
    Shortcut("copy", "Ctrl C", "复制"),
    Shortcut("paste", "Ctrl V", "粘贴"),
    Shortcut("cut", "Ctrl X", "剪切"),
    Shortcut("select_all", "Ctrl A", "全选"),
    Shortcut("undo", "Ctrl Z", "撤销"),
    Shortcut("redo", "Ctrl Y", "重做"),
    Shortcut("save", "Ctrl S", "保存"),
)

@Composable
private fun ShortcutRow(onKey: (String) -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState())
            .padding(top = 12.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        SHORTCUTS.forEach { shortcut ->
            SuggestionChip(
                onClick = { onKey(shortcut.name) },
                label = { Text(shortcut.label) },
                modifier = Modifier.semantics { contentDescription = shortcut.description },
            )
        }
    }
}

@Composable
private fun WindowScreen(
    windows: List<DesktopWindow>,
    onRefresh: () -> Unit,
    onFocus: (String) -> Unit,
    onMinimize: (String) -> Unit,
    onMaximize: (String) -> Unit,
    onBack: () -> Unit,
) {
    val refresh = rememberUpdatedState(onRefresh)
    LaunchedEffect(Unit) {
        while (true) {
            delay(REFRESH_MS)
            refresh.value()
        }
    }
    Column(modifier = Modifier.fillMaxSize()) {
        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(start = 4.dp, end = 8.dp, top = 8.dp)) {
            IconButton(onClick = onBack) {
                Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "返回")
            }
            Text("窗口", style = MaterialTheme.typography.titleLarge, modifier = Modifier.weight(1f))
            TextButton(onClick = onRefresh) { Text("刷新") }
        }
        if (windows.isEmpty()) {
            Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                Text("没有可以切换的窗口", color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        } else {
            androidx.compose.foundation.lazy.LazyColumn(modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
                items(windows.size) { index ->
                    val window = windows[index]
                    val state = listOfNotNull(
                        "置顶".takeIf { window.topmost },
                        "最小化".takeIf { window.minimized },
                        "最大化".takeIf { window.maximized },
                    ).joinToString(" · ")
                    ListItem(
                        headlineContent = { Text(window.title, maxLines = 1, fontWeight = FontWeight.Medium) },
                        supportingContent = { if (state.isNotEmpty()) Text(state) },
                        leadingContent = { WindowIcon(window.icon) },
                        trailingContent = {
                            Row {
                                IconButton(onClick = { onMinimize(window.id) }, modifier = Modifier.size(40.dp)) {
                                    Icon(WindowGlyph.Minimize, contentDescription = "最小化", modifier = Modifier.size(18.dp))
                                }
                                IconButton(onClick = { onMaximize(window.id) }, modifier = Modifier.size(40.dp)) {
                                    val restored = window.maximized
                                    Icon(
                                        if (restored) WindowGlyph.Restore else WindowGlyph.Maximize,
                                        contentDescription = if (restored) "还原" else "最大化",
                                        modifier = Modifier.size(18.dp),
                                    )
                                }
                            }
                        },
                        modifier = Modifier.clickable { onFocus(window.id) },
                        colors = androidx.compose.material3.ListItemDefaults.colors(containerColor = androidx.compose.ui.graphics.Color.Transparent),
                    )
                    HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
                }
            }
        }
    }
}

/** 窗口控制的三个小图标。Material 图标库没有这一组，按 24 的画布自己画。 */
private object WindowGlyph {
    val Minimize: ImageVector = windowGlyph("Minimize") { moveTo(5f, 18f); horizontalLineTo(19f) }
    val Maximize: ImageVector = windowGlyph("Maximize") { moveTo(6f, 6f); horizontalLineTo(18f); verticalLineTo(18f); horizontalLineTo(6f); close() }
    val Restore: ImageVector = windowGlyph("Restore") {
        moveTo(8f, 8f); horizontalLineTo(18f); verticalLineTo(18f); horizontalLineTo(8f); close()
        moveTo(6f, 6f); horizontalLineTo(16f); verticalLineTo(8f); horizontalLineTo(8f); verticalLineTo(16f); horizontalLineTo(6f); close()
    }

    private fun windowGlyph(name: String, block: PathBuilder.() -> Unit) = ImageVector.Builder(
        name = name,
        defaultWidth = 24.dp,
        defaultHeight = 24.dp,
        viewportWidth = 24f,
        viewportHeight = 24f,
    ).path(
        stroke = SolidColor(Color.Black),
        strokeLineWidth = 1.6f,
        strokeLineCap = StrokeCap.Round,
        strokeLineJoin = StrokeJoin.Round,
        pathBuilder = block,
    ).build()
}

@Composable
private fun WindowIcon(encoded: String) {
    val painter = androidx.compose.runtime.remember(encoded) {
        runCatching {
            val bytes = android.util.Base64.decode(encoded, android.util.Base64.DEFAULT)
            val bitmap = android.graphics.BitmapFactory.decodeByteArray(bytes, 0, bytes.size)
            androidx.compose.ui.graphics.painter.BitmapPainter(bitmap.asImageBitmap())
        }.getOrNull()
    }
    if (painter != null) {
        androidx.compose.foundation.Image(painter, contentDescription = null, modifier = Modifier.size(36.dp))
    } else {
        Icon(Icons.Outlined.WebAsset, contentDescription = null, modifier = Modifier.size(36.dp))
    }
}

@Composable
private fun SettingsScreen(
    mode: String,
    onMode: (String) -> Unit,
    onForget: () -> Unit,
    onBack: () -> Unit,
) {
    Column(modifier = Modifier.fillMaxSize()) {
        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(start = 4.dp, end = 20.dp, top = 8.dp)) {
            IconButton(onClick = onBack) {
                Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "返回")
            }
            Text("设置", style = MaterialTheme.typography.titleLarge)
        }
        Text(
            "输入方式",
            modifier = Modifier.padding(start = 24.dp, top = 20.dp, bottom = 8.dp),
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Card(
            modifier = Modifier.padding(horizontal = 16.dp),
            shape = MaterialTheme.shapes.large,
            colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant),
        ) {
            ListItem(
                headlineContent = { Text("粘贴", fontWeight = FontWeight.Medium) },
                supportingContent = { Text("整段贴进电脑，适合聊天和长文") },
                trailingContent = { RadioButton(selected = mode == "paste", onClick = { onMode("paste") }) },
                colors = androidx.compose.material3.ListItemDefaults.colors(containerColor = androidx.compose.ui.graphics.Color.Transparent),
            )
            HorizontalDivider(modifier = Modifier.padding(horizontal = 16.dp), color = MaterialTheme.colorScheme.outlineVariant)
            ListItem(
                headlineContent = { Text("逐字", fontWeight = FontWeight.Medium) },
                supportingContent = { Text("一个字一个字按进去，适合不支持粘贴的窗口") },
                trailingContent = { RadioButton(selected = mode == "type", onClick = { onMode("type") }) },
                colors = androidx.compose.material3.ListItemDefaults.colors(containerColor = androidx.compose.ui.graphics.Color.Transparent),
            )
        }
        Spacer(Modifier.height(24.dp))
        TextButton(
            onClick = onForget,
            modifier = Modifier.padding(horizontal = 16.dp),
            colors = androidx.compose.material3.ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.error),
        ) {
            Text("取消配对")
        }
    }
}

@Composable
private fun StatusChip(state: SessionState, status: MagpieStatus) {
    val (label, colors) = when (state) {
        SessionState.Disconnected -> "未连接" to status.waiting
        SessionState.Connecting -> "正在连接" to status.waiting
        is SessionState.Connected -> "已连接 ${state.name}".trim() to status.connected
        is SessionState.Failed -> state.message to status.failed
    }
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .background(colors.background, RoundedCornerShape(20.dp))
            .padding(horizontal = 12.dp, vertical = 6.dp),
    ) {
        Box(modifier = Modifier.size(8.dp).background(colors.foreground, RoundedCornerShape(4.dp)))
        Spacer(Modifier.width(6.dp))
        Text(label, color = colors.foreground, fontWeight = FontWeight.Medium, style = MaterialTheme.typography.labelLarge)
    }
}

/** 粘贴模式等输入停这么久再发。语音识别改字的间隔比一次按键长。 */
internal const val SETTLE_MS = 900L

/** 窗口页开着时，隔这么久重新要一次电脑上的窗口。 */
internal const val REFRESH_MS = 3_000L

/** 一次修订：删掉电脑上已发出的尾部，再补上新的。 */
internal data class TailEdit(val erase: Int, val add: String)

/**
 * 语音识别会改掉已经发出去的后半句。只退掉不相同的尾部再补新的，
 * 相同的前缀留在电脑上，不再整段重贴。
 */
internal fun tailEdit(sent: String, text: String): TailEdit? {
    if (text == sent) return null
    val keep = sent.commonPrefixWith(text).length
    return TailEdit(sent.length - keep, text.substring(keep))
}

/**
 * 逐字模式按键即发，删字发退格。
 * 粘贴模式不在这里发：等 [SETTLE_MS] 内没有新变化，再把整段粘贴一次。
 */
internal fun forwardDiff(
    previous: String,
    next: String,
    mode: String,
    onSend: (text: String, mode: String, enter: Boolean) -> Unit,
    onKey: (key: String) -> Unit,
) {
    if (mode != "type" || next == previous) return
    when {
        next.startsWith(previous) -> {
            val added = next.removePrefix(previous)
            if (added.isNotEmpty()) onSend(added, "type", false)
        }
        previous.startsWith(next) -> repeat(previous.length - next.length) { onKey("backspace") }
        else -> {
            repeat(previous.length) { onKey("backspace") }
            if (next.isNotEmpty()) onSend(next, "type", false)
        }
    }
}

/** 拼音还在组词时是小写字母，上屏后才有汉字、空格或标点。 */
private fun committed(text: String): Boolean = text.any { it !in 'a'..'z' }

private fun splitHost(raw: String): Pair<String, Int> {
    val trimmed = raw.trim()
    val parts = trimmed.split(":")
    val port = parts.getOrNull(1)?.toIntOrNull()
    return if (port != null) parts[0] to port else trimmed to 17331
}
