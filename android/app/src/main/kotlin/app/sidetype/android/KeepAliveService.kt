package app.sidetype.android

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Intent
import android.os.IBinder
import androidx.core.app.NotificationCompat

/** 连接期间的前台服务。通知只显示连接状态，避免系统杀掉套接字。 */
class KeepAliveService : Service() {
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        startForeground(NOTIFICATION_ID, notification())
        return START_STICKY
    }

    private fun notification(): Notification {
        val manager = getSystemService(NotificationManager::class.java)
        val channel = NotificationChannel(CHANNEL, "连接", NotificationManager.IMPORTANCE_MIN)
        manager.createNotificationChannel(channel)
        return NotificationCompat.Builder(this, CHANNEL)
            .setSmallIcon(android.R.drawable.stat_notify_sync)
            .setContentTitle("Sideline")
            .setContentText("已连接")
            .setOngoing(true)
            .build()
    }

    private companion object {
        const val CHANNEL = "connection"
        const val NOTIFICATION_ID = 1
    }
}
