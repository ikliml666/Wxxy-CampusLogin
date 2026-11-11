# Add project specific ProGuard rules here.
# You can control the set of applied configuration files using the
# proguardFiles setting in build.gradle.
#
# For more details, see
#   http://developer.android.com/guide/developing/tools/proguard.html

# If your project uses WebView with JS, uncomment the following
# and specify the fully qualified class name to the JavaScript interface
# class:
#-keepclassmembers class fqcn.of.javascript.interface.for.webview {
#   public *;
#}

# Uncomment this to preserve the line number information for
# debugging stack traces.
#-keepattributes SourceFile,LineNumberTable

# If you keep the line number information, uncomment this to
# hide the original source file name.
#-renamesourcefileattribute SourceFile

# Rust JNI 反向调用普通 Kotlin 方法；R8 不得重命名或移除这些接口。
-keep class com.campuslogin.plugin.monitorservice.CoreService { *; }
-keep class com.campuslogin.plugin.monitorservice.CoreEvents { *; }
-keepclassmembers class com.campuslogin.plugin.monitorservice.CoreService {
    *** emit(java.lang.String,java.lang.String);
    *** notifySystem(java.lang.String);
    *** beginProbeWindow();
    *** endProbeWindow();
    *** powerState();
    *** bindToWifi();
    *** acceptWifiNetwork();
    *** wifiSsid();
    *** reportWifiUnusable();
    *** ensureAvoidBadWifi();
    *** restoreAvoidBadWifi();
    *** startWifiWatcher();
    *** stopWifiWatcher();
    *** runIdleStop();
    *** decrypt(java.lang.String);
    *** encrypt(java.lang.String);
}
-keepclassmembers class com.campuslogin.plugin.monitorservice.CoreEvents {
    *** nativeEmitToUi(java.lang.String,java.lang.String);
}
