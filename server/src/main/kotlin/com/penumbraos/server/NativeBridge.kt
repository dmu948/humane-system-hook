package com.penumbraos.server

import android.content.Context
import android.system.Os
import android.util.Log
import java.io.BufferedReader
import java.io.File
import java.io.FileOutputStream
import java.io.InputStream
import java.io.InputStreamReader
import java.util.concurrent.TimeUnit

object NativeBridge {

    private const val TAG = "PenumbraServer"
    private const val EXECUTABLE_NAME = "libpenumbra_server_android.so"
    private const val STOP_TIMEOUT_MS = 5_000L
    private val ANSI_ESCAPE_REGEX = Regex("\\u001B\\[[;\\d]*[ -/]*[@-~]")

    fun start(context: Context, configPath: String): Process {
        val nativeLibDir = File(context.applicationInfo.nativeLibraryDir)
        val executable = File(nativeLibDir, EXECUTABLE_NAME)
        val onnxRuntime = File(nativeLibDir, "libonnxruntime.so")
        val penumbraToolHost = File(nativeLibDir, "libpenumbra_tool_host.so")

        require(executable.isFile) {
            "Server executable not found at ${executable.absolutePath}"
        }

        installPenumbraToolHost(context)

        Log.w(
            TAG,
            "Launching server executable: path=${executable.absolutePath}, canExecute=${executable.canExecute()}, config=$configPath, onnxRuntime=${onnxRuntime.absolutePath}, onnxRuntimeExists=${onnxRuntime.isFile}",
        )

        val processBuilder = ProcessBuilder(executable.absolutePath, "--config", configPath)
            .directory(File(configPath).parentFile)
            .redirectErrorStream(true)
        processBuilder.environment()["ORT_DYLIB_PATH"] = onnxRuntime.absolutePath
        processBuilder.environment()["PENUMBRA_TOOL_HOST_PATH"] = penumbraToolHost.absolutePath

        Log.w(
            TAG,
            "Native penumbra_tool_host: path=${penumbraToolHost.absolutePath}, exists=${penumbraToolHost.exists()}, canExecute=${penumbraToolHost.canExecute()}",
        )

        val process = processBuilder.start()

        process.outputStream.close()

        val processLabel = Integer.toHexString(System.identityHashCode(process))
        pumpLogs(processLabel, process.inputStream)

        Log.w(TAG, "Spawned server process id=$processLabel")
        return process
    }

    private fun installPenumbraToolHost(context: Context) {
        val outFile = File(context.filesDir, "penumbra_tool_host")
        try {
            context.assets.open("penumbra_tool_host").use { input ->
                FileOutputStream(outFile).use { output ->
                    input.copyTo(output)
                }
            }
            Os.chmod(outFile.absolutePath, 0b111000000)
            Log.w(
                TAG,
                """
                Installed penumbra_tool_host: path=${outFile.absolutePath}
                exists=${outFile.exists()}
                canRead=${outFile.canRead()}
                canWrite=${outFile.canWrite()}
                canExecute=${outFile.canExecute()}
                length=${outFile.length()}
                """.trimIndent(),
            )
        } catch (e: Exception) {
            Log.e(
                TAG,
                "Failed to install penumbra_tool_host",
                e,
            )
        }
    }

    fun stop(process: Process) {
        if (!process.isAlive) {
            return
        }

        val processLabel = Integer.toHexString(System.identityHashCode(process))
        Log.w(TAG, "Stopping server process id=$processLabel")
        process.destroy()

        if (!process.waitFor(STOP_TIMEOUT_MS, TimeUnit.MILLISECONDS)) {
            Log.w(TAG, "Server process did not exit after ${STOP_TIMEOUT_MS}ms, killing")
            process.destroyForcibly()
            process.waitFor(STOP_TIMEOUT_MS, TimeUnit.MILLISECONDS)
        }
    }

    private fun pumpLogs(processLabel: String, input: InputStream) {
        Thread({
            try {
                BufferedReader(InputStreamReader(input)).use { reader ->
                    var line: String?
                    while (true) {
                        line = reader.readLine() ?: break

                        val sanitized = line
                            ?.replace(ANSI_ESCAPE_REGEX, "")
                            ?.trimEnd()
                            .orEmpty()

                        if (sanitized.isEmpty()) {
                            continue
                        }

                        logForwardedLine(sanitized)
                    }
                }
            } catch (t: Throwable) {
                Log.w(TAG, "Server log pump ended for id=$processLabel", t)
            }
        }, "penumbra-server-logs-$processLabel").apply {
            isDaemon = true
            uncaughtExceptionHandler = Thread.UncaughtExceptionHandler { thread, error ->
                Log.e(TAG, "Uncaught on ${thread.name}", error)
            }
            start()
        }
    }

    private fun logForwardedLine(line: String) {
        when {
            line.startsWith("ERROR") || line.startsWith("error") -> Log.e(TAG, line)
            line.startsWith("WARN") || line.startsWith("warn") -> Log.w(TAG, line)
            line.startsWith("DEBUG") || line.startsWith("debug") -> Log.w(TAG, line)
            else -> Log.w(TAG, line)
        }
    }
}
