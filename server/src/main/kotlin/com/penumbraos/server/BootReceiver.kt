package com.penumbraos.server

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log

class BootReceiver : BroadcastReceiver() {

    companion object {
        private const val TAG = "PenumbraServer"
        const val ACTION_START_SERVER = "com.penumbraos.server.action.START_SERVER"
    }

    override fun onReceive(context: Context, intent: Intent) {
        try {
            if (intent.action != Intent.ACTION_BOOT_COMPLETED && intent.action != ACTION_START_SERVER) return

            Log.w(TAG, "${intent.action} received, starting foreground service")
            ServerService.start(context)
        } catch (t: Throwable) {
            Log.e(TAG, "BootReceiver.onReceive failed", t)
        }
    }
}
