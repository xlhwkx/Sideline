package app.sidetype.android.session

import android.content.Context
import android.content.SharedPreferences
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey

/** 记住的一台电脑。token 存在加密存储里。 */
data class SavedComputer(val host: String, val port: Int, val name: String, val token: String)

/** 只保存最近一台配对过的电脑。 */
class TokenStore(context: Context) {
    private val preferences: SharedPreferences = EncryptedSharedPreferences.create(
        context,
        "sidetype",
        MasterKey.Builder(context).setKeyScheme(MasterKey.KeyScheme.AES256_GCM).build(),
        EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
        EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM,
    )

    /** 没有配对过时是 null。 */
    fun load(): SavedComputer? {
        val host = preferences.getString(HOST, null) ?: return null
        val token = preferences.getString(TOKEN, null) ?: return null
        return SavedComputer(host, preferences.getInt(PORT, 17331), preferences.getString(NAME, "").orEmpty(), token)
    }

    /** 覆盖之前记住的电脑。 */
    fun save(computer: SavedComputer) {
        preferences.edit()
            .putString(HOST, computer.host)
            .putInt(PORT, computer.port)
            .putString(NAME, computer.name)
            .putString(TOKEN, computer.token)
            .apply()
    }

    /** 配对失效时清掉。 */
    fun clear() {
        preferences.edit().clear().apply()
    }

    private companion object {
        const val HOST = "host"
        const val PORT = "port"
        const val NAME = "name"
        const val TOKEN = "token"
    }
}
