package com.penumbraos.hook

import android.content.Context
import android.util.Log
import de.robv.android.xposed.XC_MethodHook
import de.robv.android.xposed.XposedBridge
import org.json.JSONObject
import java.net.HttpURLConnection
import java.net.URL
import java.util.concurrent.Callable
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit

/**
 * Inject custom reset logic into contacts
 */
object ContactsHooks {
    private const val TAG = "PenumbraContacts"
    private const val CONTACTS_PREF = "contacts"
    private const val LAST_SYNCED = "lastSynced"
    private const val LAST_SYNCED_CLOUD_SECONDS = "lastSyncedCloudSeconds"
    private const val LAST_SYNCED_CLOUD_NANO = "lastSyncedCloudNano"
    private const val RESET_CLAIM_URL = "http://127.0.0.1:8080/api/contacts/client-reset/claim"
    private val resetExecutor = Executors.newSingleThreadExecutor()

    fun install(cl: ClassLoader) {
        Log.w(TAG, "Installing contacts hooks")
        hookContactsDeltaSyncWorker(cl)
    }

    private fun hookContactsDeltaSyncWorker(cl: ClassLoader) {
        try {
            val clazz = cl.loadClass("humaneinternal.system.contacts.ContactsDeltaSyncWorker")
            val method = clazz.getDeclaredMethod("startWork")
            method.isAccessible = true

            XposedBridge.hookMethod(method, object : XC_MethodHook() {
                override fun beforeHookedMethod(param: MethodHookParam) {
                    val worker = param.thisObject ?: return
                    val context = worker.javaClass.getMethod("getApplicationContext").invoke(worker) as? Context ?: return

                    try {
                        val future = resetExecutor.submit {
                            if (claimResetFromServer()) {
                                resetContactsSynchronously(cl, context)
                            }
                        }
                        future.get(2, TimeUnit.SECONDS)
                    } catch (t: Throwable) {
                        Log.e(TAG, "Contact reset claim failed; allowing original sync to continue", t)
                    }
                }
            })
            Log.w(TAG, "  Hooked ContactsDeltaSyncWorker.startWork()")
        } catch (t: Throwable) {
            Log.e(TAG, "  Failed to hook ContactsDeltaSyncWorker.startWork(): ${t.message}")
        }
    }

    private fun claimResetFromServer(): Boolean {
        val connection = (URL(RESET_CLAIM_URL).openConnection() as HttpURLConnection).apply {
            requestMethod = "POST"
            connectTimeout = 500
            readTimeout = 500
            doOutput = true
            setRequestProperty("Content-Length", "0")
        }

        return try {
            val status = connection.responseCode
            if (status !in 200..299) {
                Log.w(TAG, "  Contact reset claim returned HTTP $status")
                false
            } else {
                val body = connection.inputStream.bufferedReader().use { it.readText() }
                JSONObject(body).optBoolean("reset", false)
            }
        } finally {
            connection.disconnect()
        }
    }

    private fun resetContactsSynchronously(cl: ClassLoader, context: Context) {
        Log.w(TAG, "Contact reset requested by server claim")

        clearContactsDatabaseSynchronously(cl)
        Log.w(TAG, "  Cleared contacts database")

        val committed = context.getSharedPreferences(CONTACTS_PREF, 0)
            .edit()
            .remove(LAST_SYNCED)
            .remove(LAST_SYNCED_CLOUD_SECONDS)
            .remove(LAST_SYNCED_CLOUD_NANO)
            .commit()
        Log.w(TAG, "  Cleared contacts sync cursor: $committed")
    }

    private fun clearContactsDatabaseSynchronously(cl: ClassLoader) {
        val managerClass = cl.loadClass("humaneinternal.system.contacts.ContactsManager")
        val manager = managerClass.getDeclaredMethod("sharedInstance").apply {
            isAccessible = true
        }.invoke(null)

        val persistenceManager = managerClass.getDeclaredField("mContactsPersistenceManager").apply {
            isAccessible = true
        }.get(manager)

        val database = persistenceManager.javaClass.getDeclaredField("mDatabase").apply {
            isAccessible = true
        }.get(persistenceManager)

        val contactsDaoMethod = database.javaClass.getMethod("contactsDao")
        val submitMethod = database.javaClass.getMethod("submit", Callable::class.java)
        val future = submitMethod.invoke(database, Callable {
            val contactsDao = contactsDaoMethod.invoke(database)
            contactsDao.javaClass.getMethod("deleteAll").invoke(contactsDao)
            null
        }) as java.util.concurrent.Future<*>

        future.get()
    }
}
