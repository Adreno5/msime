package app.msime.android.test;

import android.app.Activity;
import android.os.Build;
import android.os.Bundle;
import android.text.InputType;
import android.view.WindowInsets;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputConnectionWrapper;
import android.view.WindowInsetsController;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.TextView;
import android.content.Context;
import android.view.inputmethod.InputMethodManager;
import android.view.WindowManager;
import app.msime.android.WindowLayout;

/** Separate synthetic editor app: tests the system InputConnection, not a mock. */
public final class EditorActivity extends Activity {
    @SuppressWarnings("deprecation")
    @Override public void onCreate(Bundle state) {
        WindowLayout.theme(this);
        super.onCreate(state);
        getWindow().setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE
            | WindowManager.LayoutParams.SOFT_INPUT_STATE_ALWAYS_VISIBLE);
        LinearLayout layout = new LinearLayout(this);
        layout.setOrientation(LinearLayout.VERTICAL);
        WindowLayout.fitSystemBars(layout);
        TextView title = new TextView(this);
        title.setText("MSIME synthetic editor fixture");
        layout.addView(title);
        EditText plain = new EditText(this);
        plain.setInputType(InputType.TYPE_CLASS_TEXT);
        plain.setContentDescription("msime-test-plain");
        plain.setHint("Plain editor");
        showImeOnFocus(plain);
        layout.addView(plain);
        EditText password = new EditText(this);
        password.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD);
        password.setContentDescription("msime-test-password");
        password.setHint("Password editor");
        showImeOnFocus(password);
        layout.addView(password);
        // 系统标记为邮箱的输入框：邮箱后缀（#6147）只在这种输入框里出现。
        EditText email = new EditText(this);
        email.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS);
        email.setContentDescription("msime-test-email");
        email.setHint("Email editor");
        showImeOnFocus(email);
        layout.addView(email);
        EditText cursorAtEnd = new CursorAtEndEditText(this);
        cursorAtEnd.setInputType(InputType.TYPE_CLASS_TEXT);
        cursorAtEnd.setContentDescription("msime-test-cursor-at-end");
        cursorAtEnd.setHint("Editor that ignores newCursorPosition");
        showImeOnFocus(cursorAtEnd);
        layout.addView(cursorAtEnd);
        EditText noCursorAnchor = new NoCursorAnchorEditText(this);
        noCursorAnchor.setInputType(InputType.TYPE_CLASS_TEXT);
        noCursorAnchor.setContentDescription("msime-test-no-cursor-anchor");
        noCursorAnchor.setHint("Editor that reports no cursor position");
        showImeOnFocus(noCursorAnchor);
        layout.addView(noCursorAnchor);
        // 窄的多行输入框：一段长组字放不下一行，折到下一行（#6792 候选栏跟随光标要放在组字最后一行下方）。
        EditText wrapping = new EditText(this);
        wrapping.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_MULTI_LINE);
        wrapping.setContentDescription("msime-test-wrapping");
        wrapping.setHint("Wrap");
        showImeOnFocus(wrapping);
        layout.addView(wrapping, new LinearLayout.LayoutParams(
            Math.round(120 * getResources().getDisplayMetrics().density), LinearLayout.LayoutParams.WRAP_CONTENT));
        setContentView(layout);
    }

    /**
     * 不认 `commitText` 第二个参数的编辑器：不管输入法传多少，都当作 1，光标落在新文字后面。#6458 里 vivo「信息」和系统设置的搜索框就是这样，输入法用 `commitText(closing, 0)` 补上的后半个把光标带到了括号外面；这个输入框在模拟器上重现那种行为。
     */
    private static final class CursorAtEndEditText extends EditText {
        CursorAtEndEditText(Context context) { super(context); }

        @Override public InputConnection onCreateInputConnection(EditorInfo outAttrs) {
            InputConnection connection = super.onCreateInputConnection(outAttrs);
            if (connection == null) return null;
            return new InputConnectionWrapper(connection, false) {
                @Override public boolean commitText(CharSequence text, int newCursorPosition) {
                    return super.commitText(text, 1);
                }
            };
        }
    }

    /**
     * 不报告光标位置的编辑器：`requestCursorUpdates` 一律返回假，与自绘编辑器、部分 WebView 和游戏一样（#6792）。「物理键盘时候选栏跟随光标」在这种输入框里要退回停在底部的候选条。
     */
    private static final class NoCursorAnchorEditText extends EditText {
        NoCursorAnchorEditText(Context context) { super(context); }

        @Override public InputConnection onCreateInputConnection(EditorInfo outAttrs) {
            InputConnection connection = super.onCreateInputConnection(outAttrs);
            if (connection == null) return null;
            return new InputConnectionWrapper(connection, false) {
                @Override public boolean requestCursorUpdates(int cursorUpdateMode) {
                    return false;
                }

                // 安卓 13 起框架转给编辑器的是带过滤参数的这一个，包装类会直接交给被包装的连接，不经过上面那个。
                @Override public boolean requestCursorUpdates(int cursorUpdateMode, int cursorUpdateFilter) {
                    return false;
                }
            };
        }
    }

    private void showImeOnFocus(EditText editor) {
        editor.setOnFocusChangeListener((view, focused) -> {
            if (focused) requestIme(editor);
        });
    }

    @Override public void onWindowFocusChanged(boolean focused) {
        super.onWindowFocusChanged(focused);
        if (focused && getCurrentFocus() instanceof EditText editor) requestIme(editor);
    }

    private void requestIme(EditText editor) {
        for (long delay : new long[] {250, 750, 1500, 3000}) {
            editor.postDelayed(() -> {
                if (!editor.hasWindowFocus() || !editor.hasFocus()) return;
                InputMethodManager manager = getSystemService(InputMethodManager.class);
                if (manager != null)
                    manager.showSoftInput(editor, InputMethodManager.SHOW_IMPLICIT);
                if (Build.VERSION.SDK_INT >= 30) {
                    WindowInsetsController controller = editor.getWindowInsetsController();
                    if (controller != null) controller.show(WindowInsets.Type.ime());
                }
            }, delay);
        }
    }
}
