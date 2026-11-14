plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "com.campuslogin.plugin.monitorservice"
    compileSdk = 36

    defaultConfig {
        minSdk = 24
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }
    // Shizuku UserService 的 IRemoteShell.aidl 需要 AGP 的 aidl 支持
    buildFeatures {
        aidl = true
    }
}

dependencies {
    implementation(project(":tauri-android"))
    // FileProvider(APK 安装 content:// URI)
    implementation("androidx.core:core-ktx:1.13.1")
    // Shizuku：core 进程以 shell 身份执行 cmd wifi（用户装好 Shizuku 并授权后免电脑）。
    // api 而非 implementation —— app 模块的 CampusLoginApplication 要引用 ShizukuProvider，
    // 靠 api 传递依赖才拿得到类。
    api("dev.rikka.shizuku:api:13.1.5")
    api("dev.rikka.shizuku:provider:13.1.5")
}
