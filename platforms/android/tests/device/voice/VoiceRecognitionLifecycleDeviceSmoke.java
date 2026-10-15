package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.content.Intent;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.IntentFilter;
import android.os.Build;
import android.os.Bundle;
import android.os.SystemClock;

/** 用合成识别服务验证真实语音窗口在恢复之后才创建识别器，不访问麦克风或网络。 */
public final class VoiceRecognitionLifecycleDeviceSmoke extends Instrumentation {
    static final String CALLBACKS = "app.msime.android.test.SPEECH_READY_ERROR";
    private volatile boolean callbacksDelivered;
    private volatile String violation;
    private volatile int created;
    private volatile int resumed;

    @Override public void onCreate(Bundle arguments) {
        super.onCreate(arguments);
        start();
    }

    @Override public void callActivityOnCreate(Activity activity, Bundle state) {
        super.callActivityOnCreate(activity, state);
        if (!isVoice(activity)) return;
        created++;
        try {
            var field = activity.getClass().getDeclaredField("recognizer");
            field.setAccessible(true);
            if (field.get(activity) != null) violation = "Recognizer started before resume";
        } catch (ReflectiveOperationException error) {
            violation = "Recognizer state unavailable";
        }
    }

    @Override public void callActivityOnResume(Activity activity) {
        super.callActivityOnResume(activity);
        if (!isVoice(activity)) return;
        resumed++;
        try {
            var field = activity.getClass().getDeclaredField("recognizer");
            field.setAccessible(true);
            if (field.get(activity) == null) violation = "Recognizer did not start after resume";
        } catch (ReflectiveOperationException error) {
            violation = "Recognizer state unavailable";
        }
    }

    private static boolean isVoice(Activity activity) {
        return activity.getClass().getName().equals("app.msime.android.VoiceRecognitionActivity");
    }

    @Override public void onStart() {
        Bundle result = new Bundle();
        Activity activity = null;
        BroadcastReceiver receiver = new BroadcastReceiver() {
            @Override public void onReceive(Context context, Intent intent) {
                callbacksDelivered = true;
            }
        };
        if (Build.VERSION.SDK_INT >= 33) {
            getTargetContext().registerReceiver(receiver, new IntentFilter(CALLBACKS), Context.RECEIVER_EXPORTED);
        } else {
            getTargetContext().registerReceiver(receiver, new IntentFilter(CALLBACKS));
        }
        try {
            Intent intent = new Intent().setClassName(getTargetContext(),
                "app.msime.android.VoiceRecognitionActivity")
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
            activity = startActivitySync(intent);
            long deadline = SystemClock.uptimeMillis() + 10000;
            while ((!callbacksDelivered || (!activity.isFinishing() && !activity.isDestroyed()))
                    && SystemClock.uptimeMillis() < deadline) SystemClock.sleep(50);
            if (!callbacksDelivered) throw new AssertionError("Synthetic service did not deliver callbacks");
            if (violation != null) throw new AssertionError(violation);
            if (created != 1 || resumed != 1) throw new AssertionError("Unexpected voice lifecycle count");
            if (!activity.isFinishing() && !activity.isDestroyed())
                throw new AssertionError("Synthetic permission error did not finish the request");
            result.putString("stream", "MSIME_VOICE_LIFECYCLE_PASSED\n");
            finish(Activity.RESULT_OK, result);
        } catch (Exception | AssertionError error) {
            result.putString("stream", "MSIME_VOICE_LIFECYCLE_FAILED: " + error.getClass().getSimpleName()
                + (error instanceof AssertionError ? " " + error.getMessage() : "") + "\n");
            finish(Activity.RESULT_CANCELED, result);
        } finally {
            getTargetContext().unregisterReceiver(receiver);
            if (activity != null) {
                Activity running = activity;
                runOnMainSync(running::finish);
            }
        }
    }

}
