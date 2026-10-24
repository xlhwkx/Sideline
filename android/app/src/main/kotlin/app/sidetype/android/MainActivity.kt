package app.sidetype.android

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.core.content.ContextCompat
import app.sidetype.android.protocol.DesktopWindow
import app.sidetype.android.session.SessionState
import app.sidetype.android.session.SideSession
import app.sidetype.android.session.TokenStore
import app.sidetype.android.ui.SideTypeApp
import app.sidetype.android.ui.parsePairUri
import app.sidetype.android.ui.theme.MagpieShapes
import app.sidetype.android.ui.theme.MagpieTypography
import app.sidetype.android.ui.theme.magpieScheme
import com.journeyapps.barcodescanner.ScanContract
import com.journeyapps.barcodescanner.ScanOptions

/** 唯一的界面。网络都在 [SideSession] 里。 */
class MainActivity : ComponentActivity() {
    private lateinit var session: SideSession
    private var state: SessionState by mutableStateOf(SessionState.Disconnected)
    private var windows: List<DesktopWindow> by mutableStateOf(emptyList())

    private val scan = registerForActivityResult(ScanContract()) { result ->
        val raw = result.contents ?: return@registerForActivityResult
        val scanned = parsePairUri(raw) ?: run {
            state = SessionState.Failed("这不是 Sideline 的二维码")
            return@registerForActivityResult
        }
        if (scanned.code.length == 6) {
            session.pair(scanned.host, scanned.port, scanned.code)
            return@registerForActivityResult
        }
        pendingHost = scanned.host
        pendingPort = scanned.port
        state = SessionState.Failed("已识别 ${scanned.host}，请填写配对码")
    }

    private var pendingHost: String? = null
    private var pendingPort: Int = 17331

    private val notifications = registerForActivityResult(ActivityResultContracts.RequestPermission()) { }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        session = SideSession(
            TokenStore(this),
            { updated -> runOnUiThread { state = updated } },
            { listed -> runOnUiThread { windows = listed } },
        )
        if (Build.VERSION.SDK_INT >= 33 && ContextCompat.checkSelfPermission(this, Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED) {
            notifications.launch(Manifest.permission.POST_NOTIFICATIONS)
        }
        session.reconnect()
        setContent {
            MaterialTheme(
                colorScheme = magpieScheme(isSystemInDarkTheme()),
                typography = MagpieTypography,
                shapes = MagpieShapes,
            ) {
                SideTypeApp(
                    state = state,
                    onScan = { scan.launch(ScanOptions().setPrompt("扫描电脑上的二维码").setBeepEnabled(false)) },
                    onPair = { host, port, code -> session.pair(pendingHost ?: host, if (pendingHost != null) pendingPort else port, code) },
                    onSend = { text, mode, enter -> session.sendText(text, mode, enter) },
                    onKey = { key -> session.sendKey(key) },
                    windows = windows,
                    onRefreshWindows = { session.requestWindows() },
                    onFocus = { id -> session.focus(id); session.requestWindows() },
                    onMinimize = { id -> session.minimize(id); session.requestWindows() },
                    onMaximize = { id -> session.maximize(id); session.requestWindows() },
                    onForget = {
                        pendingHost = null
                        session.forget()
                    },
                )
            }
        }
    }

    override fun onDestroy() {
        session.stop()
        super.onDestroy()
    }
}
