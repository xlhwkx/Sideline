package app.sidetype.android.ui.theme

import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * 手机端的产品配色。界面只从这里取颜色。
 * 浅色是干净的白底加大块圆角，深色跟随系统。
 */
private val LightBackground = Color(0xFFFFFFFF)
private val LightCard = Color(0xFFF6F6F8)
private val LightCard2 = Color(0xFFF1F1F4)
private val LightLine = Color(0xFFE8E8ED)
private val LightForeground = Color(0xFF121216)
private val LightMuted = Color(0xFF8A8A93)
private val LightAccent = Color(0xFF1ED760)
private val LightAccentForeground = Color(0xFF06210F)
private val LightGreen = Color(0xFF189A58)
private val LightGreenSoft = Color(0xFFE4F6EC)
private val LightRed = Color(0xFFD64545)
private val LightRedSoft = Color(0xFFFBE9E9)
private val LightAmber = Color(0xFFB45309)
private val LightAmberSoft = Color(0xFFFBF0DF)

private val DarkBackground = Color(0xFF0E0E10)
private val DarkCard = Color(0xFF1A1A1E)
private val DarkCard2 = Color(0xFF232328)
private val DarkLine = Color(0xFF2C2C32)
private val DarkForeground = Color(0xFFF4F4F6)
private val DarkMuted = Color(0xFF8E8E97)
private val DarkAccent = Color(0xFF1ED760)
private val DarkAccentForeground = Color(0xFF06210F)
private val DarkGreen = Color(0xFF6FD99B)
private val DarkGreenSoft = Color(0xFF1E3A2B)
private val DarkRed = Color(0xFFF28B82)
private val DarkRedSoft = Color(0xFF45272A)
private val DarkAmber = Color(0xFFF2B544)
private val DarkAmberSoft = Color(0xFF3F3220)

/** 状态 pill 用的一对颜色：字和浅底。 */
data class StatusColors(val foreground: Color, val background: Color)

/** 当前主题下的状态色。 */
class MagpieStatus(dark: Boolean) {
    /** 已连接。 */
    val connected = if (dark) StatusColors(DarkGreen, DarkGreenSoft) else StatusColors(LightGreen, LightGreenSoft)

    /** 出错。 */
    val failed = if (dark) StatusColors(DarkRed, DarkRedSoft) else StatusColors(LightRed, LightRedSoft)

    /** 正在连接。 */
    val waiting = if (dark) StatusColors(DarkAmber, DarkAmberSoft) else StatusColors(LightAmber, LightAmberSoft)
}

/** 大标题收紧字距，正文按手机阅读。 */
val MagpieTypography = Typography(
    headlineLarge = TextStyle(fontSize = 34.sp, fontWeight = FontWeight.Bold, letterSpacing = (-0.8).sp),
    titleLarge = TextStyle(fontSize = 20.sp, fontWeight = FontWeight.SemiBold, letterSpacing = (-0.2).sp),
    bodyMedium = TextStyle(fontSize = 15.sp, lineHeight = 22.sp),
    bodyLarge = TextStyle(fontSize = 17.sp, lineHeight = 26.sp),
    labelLarge = TextStyle(fontSize = 15.sp, fontWeight = FontWeight.Medium),
)

/** Material 配色，全部来自上面的常量。 */
fun magpieScheme(dark: Boolean): ColorScheme = if (dark) {
    darkColorScheme(
        primary = DarkAccent,
        onPrimary = DarkAccentForeground,
        primaryContainer = DarkGreenSoft,
        onPrimaryContainer = DarkGreen,
        secondaryContainer = DarkCard2,
        onSecondaryContainer = DarkForeground,
        error = DarkRed,
        errorContainer = DarkRedSoft,
        onErrorContainer = DarkRed,
        background = DarkBackground,
        onBackground = DarkForeground,
        surface = DarkBackground,
        onSurface = DarkForeground,
        surfaceContainerHigh = DarkCard,
        surfaceVariant = DarkCard2,
        onSurfaceVariant = DarkMuted,
        outline = DarkLine,
        outlineVariant = DarkLine,
    )
} else {
    lightColorScheme(
        primary = LightAccent,
        onPrimary = LightAccentForeground,
        primaryContainer = LightGreenSoft,
        onPrimaryContainer = LightGreen,
        secondaryContainer = LightCard2,
        onSecondaryContainer = LightForeground,
        error = LightRed,
        errorContainer = LightRedSoft,
        onErrorContainer = LightRed,
        background = LightBackground,
        onBackground = LightForeground,
        surface = LightBackground,
        onSurface = LightForeground,
        surfaceContainerHigh = LightCard,
        surfaceVariant = LightCard2,
        onSurfaceVariant = LightMuted,
        outline = LightLine,
        outlineVariant = LightLine,
    )
}

/** 一套圆角，控件不再各自写死。 */
val MagpieShapes = Shapes(
    extraSmall = RoundedCornerShape(8.dp),
    small = RoundedCornerShape(12.dp),
    medium = RoundedCornerShape(16.dp),
    large = RoundedCornerShape(24.dp),
    extraLarge = RoundedCornerShape(28.dp),
)
