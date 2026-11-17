plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "com.campuslogin.plugin.networkbind"
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
}

dependencies {
    implementation(project(":tauri-android"))
    // Settings.Global 写入的 shell 通道：复用 monitor-service 的 CurfewShell（Shizuku/root），
    // 与守则共用同一个 UserService 连接，不重复 bind
    implementation(project(":tauri-plugin-campus-monitor-service"))
}
