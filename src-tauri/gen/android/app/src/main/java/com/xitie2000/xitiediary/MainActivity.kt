package com.xitie2000.xitiediary

import android.os.Bundle
import android.view.WindowInsets
import android.view.ViewGroup
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)

    // edge-to-edge 下 WebView 会延伸到系统栏底下，给内容根布局加上 insets padding
    val content = findViewById<ViewGroup>(android.R.id.content)
    content.setOnApplyWindowInsetsListener { v, insets ->
      val bars = insets.getInsets(
        WindowInsets.Type.systemBars() or WindowInsets.Type.displayCutout()
      )
      v.setPadding(bars.left, bars.top, bars.right, bars.bottom)
      insets
    }
  }
}
