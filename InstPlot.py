import sys
import os
import re
import logging
from datetime import datetime
import numpy as np
import pandas as pd
import unicodedata
from PySide6.QtWidgets import (
    QApplication, QMainWindow, QFileDialog, QWidget,
    QVBoxLayout, QHBoxLayout, QPushButton, QComboBox,
    QDialog, QTableWidget, QTableWidgetItem, QLabel, QToolBar,
    QMessageBox, QLineEdit, QTextEdit, QSpinBox, QScrollArea,
    QCheckBox, QDoubleSpinBox, QListWidget, QListWidgetItem, QColorDialog,
    QAbstractItemView, QGroupBox, QRadioButton
)
from PySide6.QtGui import QAction, QPixmap, QIcon, QColor
from PySide6.QtCore import Qt, QSize, QTimer
import shutil
import contextlib
import matplotlib.pyplot as plt
from matplotlib import font_manager
from matplotlib.backends.backend_qt5agg import FigureCanvasQTAgg as FigureCanvas
from matplotlib.patches import Rectangle
from instplot_io import (
    DataIOError,
    ExportSource,
    FittedCurve,
    prepare_export,
    read_data_file,
    write_export,
)
from instplot_history import (
    ColumnPatchCommand,
    CompositeCommand,
    DeleteFilesCommand,
    DeleteRowsCommand,
    HistoryError,
    HistoryManager,
)
from instplot_fitting import FitError, fit_values
from instplot_diagnostics import configure_logging, make_exception_hook
from instplot_dialogs import (
    choose_existing_file_mode,
    choose_export_columns,
    create_dialog_buttons,
    create_file_selection_table,
    show_error_details,
)
from instplot_processing import (
    ProcessingError,
    center_values,
    denoise_values,
    local_flatten_values,
    normalize_values,
    remove_polynomial_background,
)
from instplot_rendering import InteractiveDrawScheduler
from instplot_tasks import TaskController

# 仅在首次绘图时初始化 matplotlib 样式
_mpl_style_initialized = False
_available_font_names = None
ASYNC_FIT_POINT_THRESHOLD = 250_000
LOGGER = logging.getLogger("instplot")


def _font_families(primary, available=None):
    """Return installed named fallbacks plus a quiet generic fallback."""
    global _available_font_names
    if available is None:
        if _available_font_names is None:
            _available_font_names = {entry.name for entry in font_manager.fontManager.ttflist}
        available = _available_font_names
    families = []
    for family in (primary, "Heiti TC", "SimHei"):
        if family in available and family not in families:
            families.append(family)
    families.append("sans-serif")
    return families


def _initialize_mpl_style():
    """延迟初始化 matplotlib 样式，仅在首次绘图时调用"""
    global _mpl_style_initialized
    if _mpl_style_initialized:
        return
    _mpl_style_initialized = True

    # 使用更现代的 matplotlib 风格
    try:
        plt.style.use('seaborn-v0_8-paper')
    except Exception:
        pass

    # ===== 全局默认：主界面，不使用 LaTeX =====
    plt.rcParams.update({
        # 不启用 LaTeX
        'text.usetex': False,

        # 主字体：Times New Roman（serif）
        'font.family': _font_families('Times New Roman'),

        # mathtext
        'mathtext.fontset': 'cm',
    })

    # 统一一些 rc 参数以获得更清晰的展示
    plt.rcParams.update({
        # ===== Figure =====
        'figure.dpi': 120,
        'savefig.dpi': 600,
        'figure.facecolor': 'white',
        'savefig.facecolor': 'white',
        'savefig.bbox': 'tight',

        # ===== Axes =====
        'axes.facecolor': 'white',
        'axes.edgecolor': 'black',
        'axes.linewidth': 2.0,
        'axes.grid': True,
        'axes.grid.axis': 'both',
        'axes.labelsize': 25,

        # ===== Ticks =====
        'xtick.top': True,
        'ytick.right': True,
        'xtick.direction': 'in',
        'ytick.direction': 'in',
        'xtick.labelsize': 20,
        'ytick.labelsize': 20,
        'xtick.major.width': 2.0,
        'ytick.major.width': 2.0,
        'xtick.major.size': 8,
        'ytick.major.size': 8,
        'xtick.minor.visible': False,
        'ytick.minor.visible': False,

        # ===== Lines =====
        'lines.linewidth': 2.0,
        'lines.markersize': 6,

        # ===== Colors =====
        'text.color': 'black',
        'xtick.color': 'black',
        'ytick.color': 'black',

        # 图例
        'legend.fontsize': 20,
        'legend.frameon': True,
        'legend.framealpha': 0.8,
        'legend.loc': 'lower right', 

        # ===== Grid（备用，默认关闭）=====
        'grid.color': '#BBBBBB',
        'grid.linestyle': '--',
        'grid.linewidth': 1.0,
        'grid.alpha': 0.6,
    })
    
# 仅供出版绘图使用的 LaTeX 可用性检查（不改动全局 rcParams）
def _latex_available():
    try:
        has_latex = shutil.which("latex") is not None
        has_dvipng = shutil.which("dvipng") is not None
        has_gs = shutil.which("gs") is not None
        has_dvisvgm = shutil.which("dvisvgm") is not None
        return bool(has_latex and (has_dvipng or has_gs or has_dvisvgm))
    except Exception:
        return False


@contextlib.contextmanager
def _publish_rc_context(font_family='Times New Roman'):
    """上下文管理器：仅在出版导出时启用 LaTeX 相关 rcParams。

    如果系统找不到 LaTeX，可安全回退为不使用 LaTeX（避免报错）。
    
    Args:
        font_family: 字体族，'Helvetica' 或 'Times New Roman'
    """
    if font_family == 'Helvetica' and _latex_available():
        # Helvetica 必须使用用户指定的 LaTeX 配置（包括 sfmath）
        preamble = r"""
        \usepackage{helvet}
        \usepackage{sfmath}
        \usepackage{amsmath}
        \usepackage{upgreek}
        \usepackage{amssymb}
        """
        with plt.rc_context({
            'text.usetex': True,
            'font.family': 'sans-serif',
            'font.sans-serif': ['Helvetica'],
            'text.latex.preamble': preamble,
        }):
            yield
    else:
        # Times New Roman 不使用 LaTeX，使用普通字体渲染
        with plt.rc_context({
            'text.usetex': False,
            'font.family': 'serif',
            'font.serif': ['Times New Roman', 'Times', 'DejaVu Serif'],
        }):
            yield

# 内嵌的浅色 QSS，作为缺省/回退样式（如果外部 style_light.qss 不存在或不可读）
STYLE_LIGHT_QSS = r"""
/* 非全白的“浅色”主题：整体为深灰但不是纯黑，绘图区仍由 Matplotlib 单独控制为白色 */
QWidget { background-color: #2b3036; color: #e6eef6; }
QToolBar { background: #32363c; spacing: 6px; border-bottom: 1px solid #3a3f45; }
QPushButton { background-color: #374151; border: 1px solid #424750; border-radius: 8px; padding: 8px 12px; color: #ffffff; }
QPushButton:hover { background-color: #3f4a56; }
QPushButton:pressed { background-color: #2f3a45; }
QComboBox { padding: 6px 10px; border: 1px solid #424750; border-radius: 8px; background-color: #374151; color: #ffffff; }
QComboBox::drop-down { width: 18px; border: none; background: transparent; }
/* 小箭头提示（浅色主题为深色箭头） */
/* 使用内嵌 SVG 作为小箭头，避免 CSS 三角在某些平台显示为矩形的问题 */
QComboBox::down-arrow {
    image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='8' height='6'><polygon points='0,0 8,0 4,6' fill='%23111111'/></svg>");
    width: 8px;
    height: 6px;
    margin-right: 6px;
}
QStatusBar { background: #2b3036; color: #d0d7de; }

/* 保持表格和头部配色 */
QTableWidget { background-color: #22262a; color: #e6eef6; gridline-color: #33383d; }
QHeaderView::section { background-color: #2b3036; color: #e6eef6; }

/* 下拉弹出列表统一使用深色背景，与整体界面一致 */
QComboBox QAbstractItemView, QListView, QMenu {
    background-color: #2b3036;
    color: #e6eef6;
    selection-background-color: #3f4a56;
    outline: none;
}
QComboBox QAbstractItemView::item:selected, QListView::item:selected {
    background-color: #3f4a56;
    color: #ffffff;
}
QComboBox QAbstractItemView::item:hover {
    background-color: #374151;
    color: #ffffff;
}
"""

def latex_to_unicode(name):
    replacements = {
        r'\theta': '\u03B8',   # θ
        r'\mu': '\u03BC',      # μ
        r'\Omega': '\u03A9',   # Ω
        r'\alpha': '\u03B1',   # α
        r'\beta': '\u03B2',    # β
        r'\gamma': '\u03B3',   # γ
        r'\Delta': '\u0394',   # Δ
        r'\sigma': '\u03C3',   # σ
    }
    for k, v in replacements.items():
        name = name.replace(k, v)
    return name

class SquareFigureCanvas(FigureCanvas):
    """自定义 Canvas 类，保持绘图区域为正方形"""
    def __init__(self, figure):
        super().__init__(figure)
        from PySide6.QtWidgets import QSizePolicy
        # 设置尺寸策略，支持高度随宽度变化
        size_policy = QSizePolicy(QSizePolicy.Expanding, QSizePolicy.Expanding)
        size_policy.setHeightForWidth(True)
        self.setSizePolicy(size_policy)
    
    def hasHeightForWidth(self):
        """告诉布局系统这个控件的高度依赖于宽度"""
        return True
    
    def heightForWidth(self, width):
        """返回给定宽度所对应的高度（正方形，所以返回相同值）"""
        return width

    def sizeHint(self):
        """向布局建议一个合适的默认大小（正方形）。"""
        try:
            from PySide6.QtCore import QSize
            return QSize(800, 800)
        except Exception:
            return super().sizeHint()

    def minimumSizeHint(self):
        """给出一个较小但可用的正方形最小尺寸。"""
        try:
            from PySide6.QtCore import QSize
            return QSize(500, 500)
        except Exception:
            return super().minimumSizeHint()

class PlotApp(QMainWindow):
    def __init__(self):
        super().__init__()
        self.setWindowTitle("InstPlot")
        
        # 根据屏幕可用区域自适应窗口尺寸（保持正方形，考虑高 DPI）
        screen = QApplication.primaryScreen()
        # 使用 availableGeometry() 获取去除任务栏后的可用区域
        available = screen.availableGeometry()
        
        # 获取 DPI 缩放比例
        dpi = screen.logicalDotsPerInch()
        dpi_scale = dpi / 96.0  # 96 是标准 DPI
        
        # 根据 DPI 调整窗口占据比例
        # 标准 DPI (100%): 占高度的 2/3
        # 高 DPI (150%): 占高度的 55%（2/3 / 1.2）
        # 更高 DPI: 进一步降低比例
        if dpi_scale <= 1.0:
            height_ratio = 0.67  # 2/3
        elif dpi_scale <= 1.25:
            height_ratio = 0.60  # 125% DPI
        elif dpi_scale <= 1.5:
            height_ratio = 0.55  # 150% DPI
        else:
            height_ratio = 0.50  # 200% 及以上
        
        window_size = int(available.height() * height_ratio)
        # 确保不小于最小尺寸，也不超过可用宽度
        # 根据 DPI 设置不同的最小窗口尺寸
        min_window_size = 700 if dpi_scale <= 1.25 else 500
        window_size = max(window_size, min_window_size)
        window_size = min(window_size, available.width() - 50)  # 留 50px 边距
        self.resize(window_size, window_size)
        
        # 设置最小窗口尺寸
        self.setMinimumSize(600, 500)
        
        # 将窗口居中显示在可用区域内
        self.move(
            available.x() + (available.width() - window_size) // 2,
            available.y() + (available.height() - window_size) // 2
        )
        
        self.setAcceptDrops(True)
        self.max_history = 10  # 最多保存 10 步历史
        
        # 去噪参数记忆
        self.last_denoise_window_length = 11
        self.last_denoise_polyorder = 3
        self.last_denoise_use_range = False
        self.last_denoise_x_col = None
        self.last_denoise_x1 = 0
        self.last_denoise_x2 = 10
        
        # 行范围过滤参数
        self.row_filter_enabled = False  # 是否启用行范围过滤
        self.row_filter_mode = 'all'  # 'all', 'first_half', 'second_half', 'custom'
        self.row_filter_custom_slice = None  # 自定义切片字符串，如 ":10" 表示前10行，"10:" 表示从第10行开始
        
        self.dragging = False
        self.last_mouse_pos = None
        # 矩形选择相关
        self._rect_selector = None
        self._rect_start = None  # (xdata, ydata)
        self._mouse_press_pix = None  # (xpix, ypix)
        self._is_selecting = False

        # 状态栏
        self.statusBar().showMessage("拖入数据文件或点击打开文件按钮")

        # 初始化字体大小（会在 resizeEvent 中动态更新）
        self._update_font_sizes()

        # =============== 工具栏 ===============
        self.toolbar = QToolBar("主工具栏", self)
        self.toolbar.setIconSize(QSize(25, 25))
        self.addToolBar(self.toolbar)

        # 应用全局样式（QSS）——轻量美化：圆角按钮、统一配色、悬停效果
        app = QApplication.instance()
        if app is not None:
            # 优先尝试读取外部样式表文件 style_light.qss（放在与本文件相同目录）
            qss_path = os.path.join(os.path.dirname(__file__), 'style_light.qss')
            try:
                if os.path.exists(qss_path):
                    with open(qss_path, 'r', encoding='utf-8') as f:
                        app.setStyleSheet(f.read())
                else:
                    # 如果外部文件不存在，使用内嵌的 STYLE_LIGHT_QSS 常量作为回退
                    app.setStyleSheet(STYLE_LIGHT_QSS)
            except Exception:
                # 最后退回到非常简单的内联样式，保证应用能显示
                try:
                    qss = """
                    QWidget { background-color: #fafafa; }
                    QToolBar { background: #ffffff; spacing: 6px; border-bottom: 1px solid #e6e6e6; }
                    QPushButton { background-color: #f0f3f7; border: 1px solid #d9e1ec; border-radius: 6px; padding: 6px 10px; }
                    QPushButton:hover { background-color: #e8eef7; }
                    QPushButton:pressed { background-color: #dfeaf9; }
                    QComboBox { padding: 4px 8px; border: 1px solid #d9dfe6; border-radius: 6px; }
                    QStatusBar { background: #ffffff; }
                    """
                    app.setStyleSheet(qss)
                except Exception:
                    pass

        def make_action(icon_name, text, slot):
            """快速创建带 FontAwesome 图标的 QAction"""
            act = QAction(text, self)
            try:
                # qtawesome 需要在 QApplication 创建后才能使用
                from qtawesome import icon as qta_icon
                act.setIcon(qta_icon(icon_name, color='#5f6368'))
            except Exception as e:
                # 如果图标加载失败，只使用文本
                print(f"图标加载失败 ({icon_name}): {e}")
            act.setStatusTip(text)
            act.triggered.connect(slot)
            return act

        
        # 图标参考：https://github.com/spyder-ide/qtawesome/blob/master/qtawesome/iconic-fonts.md
        # fa5s = FontAwesome 5 Solid, fa5r = FontAwesome 5 Regular
        self.toolbar.addAction(make_action("fa5s.folder-open", "打开文件", self.open_file))
        self.toolbar.addAction(make_action("fa5s.save", "导出数据", self.export_data))
        self.toolbar.addAction(make_action("fa5s.image", "保存图片", self.save_figure))
        self.toolbar.addAction(make_action("fa5s.undo", "撤回", self.undo))
        self.toolbar.addAction(make_action("fa5s.redo", "重做", self.redo))
        self.toolbar.addAction(make_action("fa5s.edit", "输入数据", self.open_input_dialog))
        self.toolbar.addAction(make_action("fa5s.chart-line", "拟合曲线", self.open_fit_dialog))
        self.toolbar.addAction(make_action("fa5s.sliders-h", "图例设置", self.open_legend_config))
        self.toolbar.addAction(make_action("fa5s.trash-alt", "删除线条", self.open_delete_line_dialog))
        self.toolbar.addAction(make_action("fa5s.file-alt", "出版绘图", self.open_publish_dialog))
        self.toolbar.addSeparator()
        # 主题（仅浅色），不提供深色切换

        # 画布和工具栏（使用轻量创建方式）
        from matplotlib.figure import Figure
        self.figure = Figure(figsize=(7.0, 7.0), dpi=100, facecolor='white')
        self.ax = self.figure.add_subplot(111, facecolor='white')
        self.canvas = SquareFigureCanvas(self.figure)
        self.interactive_draws = InteractiveDrawScheduler(self.canvas.draw_idle, self)
        # 固定 canvas 显示尺寸，不随窗口拉伸
        #self.canvas.setFixedSize(int(7.5 * 100), int(7.0 * 100))  # dpi=100

        self.canvas.mpl_connect('motion_notify_event', self.on_mouse_move)
        self.canvas.mpl_connect('button_press_event', self.on_mouse_press)
        # 鼠标交互绑定
        self.canvas.mpl_connect("button_release_event", self.on_mouse_release)
        self.canvas.mpl_connect("motion_notify_event", self.on_mouse_drag)
        self.canvas.mpl_connect("scroll_event", self.on_scroll)

        # =============== 下拉菜单和绘制按钮 ===============
        self.btn_denoise = QPushButton("去噪")
        self.btn_local_detrend = QPushButton("局部处理")
        self.btn_row_filter = QPushButton("行范围选择")
        self.btn_center = QPushButton("对称处理")
        self.btn_normalize = QPushButton("归一化")
        self.btn_remove_bg = QPushButton("去背底")
        self.btn_clear = QPushButton("清空图形")
        # Matplotlib 核心导航按钮
        self.btn_save = QPushButton("保存图片")
        
        # 下拉选择（样式将在 _update_button_styles 中动态设置）
        self.combo_x = QComboBox()
        self.combo_y = QComboBox()
        
        # 防止首次加载时被右侧工具栏或其他控件遮挡，设置一个合理的最小/固定宽度
        try:
            # 使下拉宽度与“清空图形”按钮宽度一致，视觉更紧凑
            clear_w = self.btn_clear.sizeHint().width()
            if clear_w and clear_w > 0:
                w = int(clear_w)
            else:
                w = 110
            self.combo_x.setMinimumWidth(w)
            self.combo_y.setMinimumWidth(w)
            # 设置下拉列表视图的最小宽度
            self.combo_x.view().setMinimumWidth(200)
            self.combo_y.view().setMinimumWidth(200)
        except Exception:
            try:
                self.combo_x.setMinimumWidth(120)
                self.combo_y.setMinimumWidth(120)
            except Exception:
                pass
        # 不使用 X/Y 标签，保持简洁的下拉控件
        
        self.btn_plot = QPushButton("绘制曲线")

        # 顶部布局
        top_layout = QHBoxLayout()
        for w in [self.btn_denoise, self.btn_local_detrend, self.btn_row_filter, self.btn_center, self.btn_normalize,
                  self.btn_remove_bg]:
            top_layout.addWidget(w)

        top_layout.addStretch()
        for w in [self.btn_clear,self.combo_x, self.combo_y, self.btn_plot]:
            top_layout.addWidget(w)

        # =============== 主体布局 ===============
        layout = QVBoxLayout()
        layout.addLayout(top_layout)
        # 创建容器让 canvas 居中
        canvas_container = QWidget()
        canvas_layout = QVBoxLayout(canvas_container)
        canvas_layout.setContentsMargins(0, 0, 0, 0)
        canvas_layout.setAlignment(Qt.AlignCenter)
        canvas_layout.addWidget(self.canvas)
        layout.addWidget(canvas_container)

        container = QWidget()
        container.setLayout(layout)
        self.setCentralWidget(container)

        # 初始化按钮样式（在所有控件创建后）
        self._update_button_styles()

        # =============== 按钮功能绑定 ===============
        self.btn_denoise.clicked.connect(self.apply_denoise)
        self.btn_local_detrend.clicked.connect(self.apply_local_detrend)
        self.btn_row_filter.clicked.connect(self.open_row_filter_dialog)
        self.btn_center.clicked.connect(self.apply_center)
        self.btn_normalize.clicked.connect(self.apply_normalize)
        self.btn_remove_bg.clicked.connect(self.remove_background)
        self.btn_clear.clicked.connect(self.clear_plot)
        self.btn_plot.clicked.connect(self.plot_selected)

        # 数据存储
        self.loaded_files = []  # 存储 (file_path, df)
        self.history = HistoryManager(self.loaded_files, max_steps=self.max_history)
        self.tasks = TaskController(self, max_threads=1)
        self.col_unicode_map = {}
        self.last_x_col = ""
        self.last_y_col = ""
        self.placeholder_active = False  # 是否处于示例数据状态
        self.input_counter = 0  # 记录手工输入数据的编号

        # 主界面默认绘图配置（用于示例曲线和初始绘制）
        self.settings_state = {
            'selected_files': set(),
            'linestyle': '-',
            'marker': 'o',
            'markersize': 5.0,
            'linewidth': 2.0,
            'color': '#1f77b4',
            'color_scheme_type': '固定颜色',
            'colormap': 'viridis',
            'xlabel': '',
            'ylabel': '',
            'fontsize': 16,
            'fontfamily': 'Times New Roman',
            'use_latex': False,
            'xlabel_pad': 3,
            'ylabel_pad': 0,
            'tick_dir': 'in',
            'tick_len': 6.0,
            'tick_wid': 2.0,
            'tick_label_size': 14,
            'tick_axis': 'both',
            'minor_ticks': False,
            'minor_x_interval': '',
            'minor_y_interval': '',
            'per_series_style': {},
            'frame_width': 2.0,
            'radian_mode': False,
            'legend_fontsize': 14,
            'legend_loc': 'lower right',
            'legend_labels': {},
            'major_x_interval': '',
            'major_y_interval': '',
            'x_min': '',
            'x_max': '',
            'y_min': '',
            'y_max': '',
            'fig_w': 7.5,
            'fig_h': 7.0,
            'enable_twinx': False,
            'enable_twiny': False,
            'x2col': '',
            'y2col': '',
            'x2label': '',
            'y2label': '',
            'x2_min': '',
            'x2_max': '',
            'y2_min': '',
            'y2_max': '',
        }

        # 初始载入示例颜色曲线，启动即显示七条平滑曲线
        self._load_placeholder_color_demo()

        # 存储拟合曲线
        self.fitted_lines = []  # 存储拟合曲线的 Line2D 对象

        # 图例配置（用户可自定义）
        self.legend_config = {
            "show": True,
            "loc": "upper right",
            "fontsize": 20,
            "frameon": True,
            "framealpha": 0.8,
        }
        # 当前X轴单位模式：None表示原始单位，'radian'表示弧度
        self.x_unit_mode = None
        # 固定宽度，不需要自适应调整

    def _update_font_sizes(self):
        """根据窗口大小动态计算字体大小"""
        try:
            # 基于窗口宽度计算字体大小
            window_width = self.width()
            # 基础字号：窗口宽度的 1.5%，范围 11-16pt
            self.base_font_size = max(11, min(16, int(window_width * 0.015)))
            # 按钮字号：比基础字号大 1-2pt，范围 12-18pt
            self.button_font_size = max(12, min(18, self.base_font_size + 2))
        except Exception:
            self.base_font_size = 12
            self.button_font_size = 14

    def _update_button_styles(self):
        """更新所有按钮和控件的样式"""
        try:
            # 计算 padding
            padding_v = max(int(self.button_font_size * 0.4), 4)
            padding_h = max(int(self.button_font_size * 0.7), 8)
            
            # 更新按钮样式
            self.top_button_style = (
                f"font-size: {self.button_font_size}pt; font-weight: 600; "
                f"padding: {padding_v}px {padding_h}px; "
                "border-radius: 8px; background-color: #f0f3f7; border: 1px solid #d9e1ec; color: #111111;"
            )
            
            # 应用到所有按钮
            for btn in [self.btn_denoise, self.btn_local_detrend, self.btn_row_filter, self.btn_center, self.btn_normalize, self.btn_remove_bg, 
                       self.btn_clear, self.btn_save, self.btn_plot]:
                btn.setStyleSheet(self.top_button_style)
            
            # 更新下拉框样式
            combo_padding_v = max(int(self.base_font_size * 0.4), 5)
            combo_padding_h = max(int(self.base_font_size * 0.8), 10)
            self.combo_style_light = (
                f"QComboBox {{ font-size: {self.base_font_size}pt; "
                f"padding: {combo_padding_v}px {combo_padding_h}px; border-radius: 8px; "
                "background-color: #f0f3f7; border: 1px solid #d9e1ec; color: #111111; }"
                "QComboBox::drop-down { subcontrol-origin: padding; subcontrol-position: top right; width: 30px; "
                "border: none; border-left: 1px solid #d9e1ec; border-top-right-radius: 8px; border-bottom-right-radius: 8px; "
                "background-color: transparent; }"
            )
            self.combo_x.setStyleSheet(self.combo_style_light)
            self.combo_y.setStyleSheet(self.combo_style_light)
        except Exception as e:
            pass

    def resizeEvent(self, event):
        """窗口大小改变时重新计算字体大小"""
        super().resizeEvent(event)
        try:
            self._update_font_sizes()
            self._update_button_styles()
        except Exception:
            pass

    def _calculate_scaled_font_size(self, base_size):
        """根据屏幕 DPI 计算缩放后的字体大小（单位：pt）"""
        try:
            screen = QApplication.primaryScreen()
            # 获取逻辑 DPI（通常是 96 on Windows, 72 on macOS）
            dpi = screen.logicalDotsPerInch()
            # 标准 DPI 是 96
            scale_factor = dpi / 96.0
            # 返回缩放后的字体大小，确保至少为基础大小
            return max(int(base_size * scale_factor), base_size)
        except Exception:
            return base_size

    # 鼠标移动时，显示坐标
    def on_mouse_move(self, event):
        if event.inaxes:  # 鼠标在绘图区内
            x, y = event.xdata, event.ydata
            self.statusBar().showMessage(f"x={x:.4g}, y={y:.4g}")
        else:
            self.statusBar().clearMessage()

    def on_click_point(self, event):
        if event.inaxes is None or event.button != 1:  # 只响应左键
            return

        x, y = event.xdata, event.ydata
        print(f"点击坐标: x={x:.3f}, y={y:.3f}")

        # 移除上一次高亮点
        if hasattr(self, "_highlight") and self._highlight is not None:
            try:
                self._highlight.remove()
            except Exception:
                pass
            self._highlight = None
        # 在所有已加载的曲线数据中寻找距离点击点最近的点
        nearest = None
        nearest_dist = None
        nearest_info = None  # (file_index, idx_in_df, xcol, ycol)
        xcol = self.combo_x.currentText()
        ycol = self.combo_y.currentText()
        if not xcol or not ycol:
            # 如果未选择列，直接显示点击高亮点
            self._highlight = event.inaxes.plot(
                x, y, marker='o', markersize=8, color='red', markeredgecolor='black', zorder=10
            )[0]
            self.canvas.draw()
            return

        # 使用像素坐标比较（更加符合可视上的点击定位），并设置最大像素容限
        tol_pixels = 10  # 容忍范围(px)，可调整
        event_xpix = getattr(event, 'x', None)
        event_ypix = getattr(event, 'y', None)
        if event_xpix is None or event_ypix is None:
            # 回退到数据坐标方式
            event_xpix = None
        for fi, (file_path, df) in enumerate(self.loaded_files):
            if xcol not in df.columns or ycol not in df.columns:
                continue
            xs = pd.to_numeric(df[xcol], errors='coerce')
            ys = pd.to_numeric(df[ycol], errors='coerce')
            valid_mask = ~(xs.isna() | ys.isna())
            if not valid_mask.any():
                continue
            xs_v = xs[valid_mask].to_numpy()
            ys_v = ys[valid_mask].to_numpy()
            try:
                if event_xpix is not None:
                    pts_disp = self.ax.transData.transform(np.column_stack((xs_v, ys_v)))
                    dx = pts_disp[:, 0] - event_xpix
                    dy = pts_disp[:, 1] - event_ypix
                    dists = np.hypot(dx, dy)
                    min_idx = int(np.argmin(dists))
                    min_dist_pix = float(dists[min_idx])
                    if min_dist_pix <= tol_pixels and (nearest_dist is None or min_dist_pix < nearest_dist):
                        nearest_dist = min_dist_pix
                        nearest = (xs_v[min_idx], ys_v[min_idx])
                        valid_positions = np.flatnonzero(valid_mask.to_numpy())
                        nearest_info = (fi, int(valid_positions[min_idx]))
                else:
                    # 如果没有像素坐标，回退到数据坐标距离
                    dx = xs_v - x
                    dy = ys_v - y
                    dists = np.hypot(dx, dy)
                    min_idx = int(np.argmin(dists))
                    min_dist = float(dists[min_idx])
                    if nearest_dist is None or min_dist < nearest_dist:
                        nearest_dist = min_dist
                        nearest = (xs_v[min_idx], ys_v[min_idx])
                        valid_positions = np.flatnonzero(valid_mask.to_numpy())
                        nearest_info = (fi, int(valid_positions[min_idx]))
            except Exception:
                continue

        if nearest is None:
            # 无数据点可选，直接绘制点击点
            self._highlight = event.inaxes.plot(
                x, y, marker='o', markersize=8, color='red', markeredgecolor='black', zorder=10
            )[0]
            self.canvas.draw()
            return

        # 高亮最近点
        hx, hy = nearest
        try:
            self._highlight = event.inaxes.plot(
                hx, hy, marker='o', markersize=10, color='red', markeredgecolor='black', zorder=12
            )[0]
        except Exception:
            self._highlight = None
        self.canvas.draw()

        # 弹出确认框
        try:
            mb = QMessageBox(self)
            mb.setWindowTitle("删除点")
            mb.setText(f"检测到最近点 (x={hx:.4g}, y={hy:.4g})，是否删除？")
            mb.setStandardButtons(QMessageBox.Yes | QMessageBox.No)
            ret = mb.exec()
            if ret == QMessageBox.Yes:
                # 从对应 DataFrame 删除该行并重绘
                try:
                    fi, row_position = nearest_info
                    self._commit_row_deletions({fi: [row_position]})
                    # 删除点后保留当前缩放/平移状态
                    self.replot_all(preserve_view=True)
                    self.statusBar().showMessage(f"已删除点 (x={hx:.4g}, y={hy:.4g})")
                except Exception as e:
                    print("删除点失败:", e)
                    self.statusBar().showMessage("删除点失败")
            else:
                # 如果取消，移除高亮
                try:
                    if hasattr(self, '_highlight') and self._highlight is not None:
                        self._highlight.remove()
                        self._highlight = None
                        self.canvas.draw()
                except Exception:
                    pass
        except Exception:
            pass

    # 鼠标按下
    def on_mouse_press(self, event):
        # 右键开始平移
        if event.button == 3 and event.inaxes:
            self.dragging = True
            self.last_mouse_pos = (event.x, event.y)
            return

        # 左键：可能是单击也可能是矩形选择，记录起点（像素与数据坐标）
        if event.button == 1 and event.inaxes:
            try:
                self._mouse_press_pix = (event.x, event.y)
                self._rect_start = (event.xdata, event.ydata)
                self._is_selecting = False
            except Exception:
                self._mouse_press_pix = None
                self._rect_start = None
                self._is_selecting = False

    # 鼠标释放
    def on_mouse_release(self, event):
        # 结束右键平移
        if event.button == 3:
            self.dragging = False
            self.last_mouse_pos = None
            self.interactive_draws.flush()
            return

        # 左键松开：处理矩形选择结束或单击
        if event.button == 1:
            # 如果处于矩形选择中，完成批量删除流程
            if self._is_selecting and self._rect_selector is not None:
                try:
                    bbox = self._rect_selector.get_bbox()
                    xmin, ymin = bbox.x0, bbox.y0
                    xmax, ymax = bbox.x1, bbox.y1
                except Exception:
                    xmin = ymin = xmax = ymax = None

                # 移除矩形补丁
                try:
                    self._rect_selector.remove()
                except Exception:
                    pass
                self._rect_selector = None
                self._is_selecting = False
                self.interactive_draws.request()

                if xmin is None:
                    self._mouse_press_pix = None
                    self._rect_start = None
                    return

                xcol = self.combo_x.currentText()
                ycol = self.combo_y.currentText()
                if not xcol or not ycol:
                    self.statusBar().showMessage("请先选择 X/Y 列以进行区域删除")
                    self._mouse_press_pix = None
                    self._rect_start = None
                    return

                to_delete = {}
                total_count = 0
                for fi, (file_path, df) in enumerate(self.loaded_files):
                    if xcol not in df.columns or ycol not in df.columns:
                        continue
                    xs = pd.to_numeric(df[xcol], errors='coerce')
                    ys = pd.to_numeric(df[ycol], errors='coerce')
                    valid_mask = ~(xs.isna() | ys.isna())
                    if not valid_mask.any():
                        continue
                    mask_in = valid_mask & xs.between(xmin, xmax) & ys.between(ymin, ymax)
                    if mask_in.any():
                        positions = np.flatnonzero(mask_in.to_numpy()).tolist()
                        to_delete[fi] = positions
                        total_count += len(positions)

                if total_count == 0:
                    self.statusBar().showMessage("矩形内未找到数据点")
                    self._mouse_press_pix = None
                    self._rect_start = None
                    try:
                        self.canvas.draw()
                    except Exception:
                        pass
                    return

                try:
                    mb = QMessageBox(self)
                    mb.setWindowTitle("删除多个点")
                    mb.setText(f"检测到 {total_count} 个点在选区内，是否删除？")
                    mb.setStandardButtons(QMessageBox.Yes | QMessageBox.No)
                    ret = mb.exec()
                    if ret == QMessageBox.Yes:
                        try:
                            self._commit_row_deletions(to_delete)
                            # 批量删除后保留当前视图范围
                            self.replot_all(preserve_view=True)
                            self.statusBar().showMessage(f"已删除选区内 {total_count} 个点")
                        except Exception as e:
                            print("批量删除失败:", e)
                            self.statusBar().showMessage("批量删除失败")
                    else:
                        self.statusBar().showMessage("已取消批量删除")
                except Exception:
                    pass

                self._mouse_press_pix = None
                self._rect_start = None
                return

            # 否则按下与释放位置接近，视为单击
            try:
                if self._mouse_press_pix is not None:
                    px0, py0 = self._mouse_press_pix
                    if np.hypot(event.x - px0, event.y - py0) < 6:
                        self.on_click_point(event)
            except Exception:
                pass

            self._mouse_press_pix = None
            self._rect_start = None

    # 鼠标拖动
    def on_mouse_drag(self, event):
        # 右键平移优先
        if self.dragging:
            if not event.inaxes or self.last_mouse_pos is None:
                return
            dx = event.x - self.last_mouse_pos[0]
            dy = event.y - self.last_mouse_pos[1]
            ax = event.inaxes
            xlim = ax.get_xlim()
            ylim = ax.get_ylim()
            x_range = xlim[1] - xlim[0]
            y_range = ylim[1] - ylim[0]
            width, height = self.canvas.width(), self.canvas.height()
            dx_data = -dx * x_range / width
            dy_data = -dy * y_range / height
            ax.set_xlim(xlim[0] + dx_data, xlim[1] + dx_data)
            ax.set_ylim(ylim[0] + dy_data, ylim[1] + dy_data)
            self.interactive_draws.request()
            self.last_mouse_pos = (event.x, event.y)
            return

        # 左键矩形选择处理
        if self._mouse_press_pix is None or not event.inaxes:
            return
        try:
            px0, py0 = self._mouse_press_pix
            cur_px, cur_py = event.x, event.y
            dist = np.hypot(cur_px - px0, cur_py - py0)
            start_xdata, start_ydata = self._rect_start if self._rect_start is not None else (None, None)
            if dist > 6 and start_xdata is not None:
                x0, y0 = start_xdata, start_ydata
                x1, y1 = event.xdata, event.ydata
                if x1 is None or y1 is None:
                    return
                xmin, xmax = sorted([x0, x1])
                ymin, ymax = sorted([y0, y1])
                if not self._is_selecting:
                    try:
                        self._rect_selector = Rectangle((xmin, ymin), xmax - xmin, ymax - ymin,
                                                        fill=False, edgecolor='red', linewidth=1.2,
                                                        linestyle='--', zorder=11)
                        self.ax.add_patch(self._rect_selector)
                        self._is_selecting = True
                    except Exception:
                        self._rect_selector = None
                        self._is_selecting = False
                else:
                    try:
                        self._rect_selector.set_xy((xmin, ymin))
                        self._rect_selector.set_width(xmax - xmin)
                        self._rect_selector.set_height(ymax - ymin)
                    except Exception:
                        pass
                try:
                    self.interactive_draws.request()
                except Exception:
                    pass
        except Exception:
            pass

    def on_scroll(self, event):
        # 滚轮缩放
        if not event.inaxes:
            return
        xlim = self.ax.get_xlim()
        ylim = self.ax.get_ylim()
        xdata, ydata = event.xdata, event.ydata

        scale_factor = 1.1 if event.button == 'down' else 1/1.1
        new_width = (xlim[1] - xlim[0]) * scale_factor
        new_height = (ylim[1] - ylim[0]) * scale_factor

        relx = (xdata - xlim[0]) / (xlim[1] - xlim[0])
        rely = (ydata - ylim[0]) / (ylim[1] - ylim[0])

        self.ax.set_xlim([xdata - new_width * relx, xdata + new_width * (1 - relx)])
        self.ax.set_ylim([ydata - new_height * rely, ydata + new_height * (1 - rely)])
        self.interactive_draws.request()
    
    # 保存图片
    def save_figure(self):
        fname, _ = QFileDialog.getSaveFileName(
            self,
            "保存图片",
            "",
            "PNG 文件 (*.png);;JPG 文件 (*.jpg *.jpeg);;TIFF 文件 (*.tif *.tiff);;BMP 文件 (*.bmp);;PDF 文件 (*.pdf);;SVG 文件 (*.svg);;所有文件 (*)"
        )
        if fname:
            try:
                self.canvas.figure.savefig(fname, dpi=600)
                self.statusBar().showMessage(f"图片已保存: {fname}")
            except Exception as e:
                self.statusBar().showMessage(f"保存失败: {e}")

    
    # 打开文件
    def open_file(self):
        file_path, _ = QFileDialog.getOpenFileName(
            self,
            "选择数据文件",
            "",
            "Data Files (*.txt *.csv *.dat *.xls *.xlsx);;Text Files (*.txt *.csv *.dat);;Excel Files (*.xls *.xlsx);;All Files (*)"
        )
        if file_path:
            if getattr(self, "placeholder_active", False):
                # 进入真实数据前清空示例
                self.loaded_files.clear()
                self.history.reset(self.loaded_files)
                try:
                    self.combo_x.clear()
                    self.combo_y.clear()
                except Exception:
                    pass
                self.placeholder_active = False
            self.load_file_async(file_path)

    def open_input_dialog(self):
        """打开数据输入对话框，支持粘贴/手输并解析为 DataFrame。"""
        dlg = QDialog(self)
        dlg.setWindowTitle("输入数据（粘贴/手动）")
        dlg.resize(620, 560)

        # 共享状态（使用字典保存，避免控件被对话框销毁）
        self.settings_state = {
            'selected_files': {os.path.basename(self.loaded_files[0][0])} if self.loaded_files else set(),
            'linestyle': '-',
            'marker': 'o',
            'markersize': 6.0,
            'linewidth': 2.0,
            'color': '#1f77b4',
            'color_scheme_type': '固定颜色',  # 颜色方案类型：'固定颜色' 或 '渐变色'
            'colormap': 'viridis',  # 渐变色方案
            'xlabel': 'XLabel',
            'ylabel': 'YLabel',
            'fontsize': 28,
            'fontfamily': 'Times New Roman',
            # LaTeX 使用状态：按最新要求全局禁用
            'use_latex': False,
            'xlabel_pad': 3,
            'ylabel_pad': 0,
            'tick_dir': 'in',
            'tick_len': 8.0,
            'tick_wid': 2.0,
            'tick_label_size': 24,
            'tick_axis': 'both',
            'minor_ticks': False,
            'minor_x_interval': '',
            'minor_y_interval': '',
            'per_series_style': {},  # 每条曲线的独立样式
            'frame_width': 2.0,
            'radian_mode': (self.x_unit_mode == 'radian'),
            'legend_fontsize': 26,
            # legend_fontfamily 已删除，改为使用全局 fontfamily
            'legend_loc': 'lower right',
            'legend_labels': {},  # 存储自定义图例标签 {文件名: 自定义标签}
            'major_x_interval': '',
            'major_y_interval': '',
            'x_min': '',
            'x_max': '',
            'y_min': '',
            'y_max': '',
            'fig_w': 7.5,
            'fig_h': 7.0,
            # 双轴开关（默认关闭）
            'enable_twinx': False,
            'enable_twiny': False,
            'x2col': '',  # 顶部 X 列
            'y2col': '',  # 右侧 Y 列
            'x2label': '',
            'y2label': '',
            'x2_min': '',
            'x2_max': '',
            'y2_min': '',
            'y2_max': '',
        }

        # 使用可滚动区域避免界面拥挤
        root_layout = QVBoxLayout(dlg)
        scroll = QScrollArea()
        scroll.setWidgetResizable(True)
        root_layout.addWidget(scroll)
        content_widget = QWidget()
        content_widget.setStyleSheet("font-size: 14px;")
        scroll.setWidget(content_widget)
        main_layout = QVBoxLayout(content_widget)

        # 说明
        info_label = QLabel("粘贴或手动输入数据：支持逗号/空格/分号（含中文分号）/制表符分隔，系统会自动将粘贴内容打散为一列值；左右两侧分别作为 X 与 Y。")
        info_label.setWordWrap(True)
        main_layout.addWidget(info_label)

        # 列名与数据输入区（分为 X 和 Y 两列）
        col_form = QHBoxLayout()
        # 读取上次使用的列名（若存在）
        x_name_edit = QLineEdit(getattr(self, "_last_input_xname", "x"))
        y_name_edit = QLineEdit(getattr(self, "_last_input_yname", "y"))
        col_form.addWidget(QLabel("X 列名:"))
        col_form.addWidget(x_name_edit)
        col_form.addSpacing(12)
        col_form.addWidget(QLabel("Y 列名:"))
        col_form.addWidget(y_name_edit)
        col_form.addStretch()
        main_layout.addLayout(col_form)

        xy_layout = QHBoxLayout()
        text_x = QTextEdit()
        text_y = QTextEdit()
        # 修改占位提示文案（支持粘贴多列，自动拆分）
        text_x.setPlaceholderText("粘贴或输入 X 数据：逗号/空格/分号(含中文)/Tab/中文逗号自动拆分")
        text_y.setPlaceholderText("粘贴或输入 Y 数据：逗号/空格/分号(含中文)/Tab/中文逗号自动拆分")
        # 读取上次输入的内容（若存在）
        text_x.setPlainText(getattr(self, "_last_input_text_x", ""))
        text_y.setPlainText(getattr(self, "_last_input_text_y", ""))
        xy_layout.addWidget(text_x)
        xy_layout.addWidget(text_y)
        main_layout.addLayout(xy_layout)

        # X 自动生成器：输入范围和点数，一键填充 X 列
        gen_layout = QHBoxLayout()
        gen_layout.addWidget(QLabel("自动生成 X: 起止"))
        x_range_edit = QLineEdit(getattr(self, "_last_auto_x_range", "-10,10"))
        x_range_edit.setPlaceholderText("如 -10,10 或 0,2*pi 或 -pi/2,pi/2")
        gen_layout.addWidget(x_range_edit)
        gen_layout.addSpacing(8)
        gen_layout.addWidget(QLabel("点数"))
        x_points_spin = QSpinBox()
        x_points_spin.setRange(2, 200000)
        x_points_spin.setSingleStep(50)
        x_points_spin.setValue(getattr(self, "_last_auto_x_points", 200))
        gen_layout.addWidget(x_points_spin)
        btn_gen_x = QPushButton("生成 X")
        gen_layout.addWidget(btn_gen_x)
        gen_layout.addStretch()
        main_layout.addLayout(gen_layout)

        # 模板按钮（快速清空并写入默认列名占位）
        tmpl_layout = QHBoxLayout()
        btn_fill_xy = QPushButton("清空并重置 x/y")
        tmpl_layout.addWidget(btn_fill_xy)
        tmpl_layout.addStretch()
        main_layout.addLayout(tmpl_layout)

        # 预览表格
        preview = QTableWidget()
        preview.setRowCount(0)
        preview.setColumnCount(0)
        main_layout.addWidget(preview)

        # 列选择
        choose_layout = QHBoxLayout()
        x_label = QLabel("X 列：")
        y_label = QLabel("Y 列：")
        x_combo = QComboBox()
        y_combo = QComboBox()
        x2_label = QLabel("顶部 X 列：")
        y2_label = QLabel("右侧 Y 列：")
        x2_combo = QComboBox(); y2_combo = QComboBox()
        choose_layout.addWidget(x_label)
        choose_layout.addWidget(x_combo)
        choose_layout.addSpacing(12)
        choose_layout.addWidget(y_label)
        choose_layout.addWidget(y_combo)
        choose_layout.addSpacing(12)
        choose_layout.addWidget(x2_label)
        choose_layout.addWidget(x2_combo)
        choose_layout.addSpacing(12)
        choose_layout.addWidget(y2_label)
        choose_layout.addWidget(y2_combo)
        choose_layout.addStretch()
        main_layout.addLayout(choose_layout)

        # 默认隐藏双轴列选择，按全局开关显示
        x2_label.setVisible(self.settings_state.get('enable_twiny', False) or self.settings_state.get('enable_twinx', False))
        x2_combo.setVisible(self.settings_state.get('enable_twiny', False) or self.settings_state.get('enable_twinx', False))
        y2_label.setVisible(self.settings_state.get('enable_twiny', False))
        y2_combo.setVisible(self.settings_state.get('enable_twiny', False))

        # 函数生成区（可折叠，可选）：输入 f(x)，范围和点数，生成数据
        func_toggle = QPushButton("展开函数生成 (可选)")
        func_toggle.setCheckable(True)
        func_toggle.setChecked(False)
        main_layout.addWidget(func_toggle)

        func_container = QWidget()
        func_container.setVisible(False)
        func_layout = QVBoxLayout(func_container)
        func_layout.setContentsMargins(0, 0, 0, 0)
        func_title = QLabel("输入单变量函数 f(x) 自动生成数据")
        func_title.setStyleSheet("font-weight: 600;")
        func_layout.addWidget(func_title)

        func_help = QLabel("支持: + - * / ^, sin cos tan sinh cosh tanh exp log log10 sqrt abs arctan arctan2, 常量: pi, e, 变量: x。禁止多变量/自定义函数/属性访问。")
        func_help.setWordWrap(True)
        func_layout.addWidget(func_help)

        func_row1 = QHBoxLayout()
        func_row1.addWidget(QLabel("f(x):"))
        func_edit = QLineEdit(getattr(self, "_last_func_expr", "sin(x)"))
        func_row1.addWidget(func_edit)
        func_row1.addStretch()
        func_layout.addLayout(func_row1)

        func_row2 = QHBoxLayout()
        func_row2.addWidget(QLabel("x 最小:"))
        x_min_edit = QLineEdit(str(getattr(self, "_last_func_xmin", -10)))
        x_min_edit.setPlaceholderText("如 -10 或 -pi/2 或 -pi")
        func_row2.addWidget(x_min_edit)
        func_row2.addSpacing(8)
        func_row2.addWidget(QLabel("x 最大:"))
        x_max_edit = QLineEdit(str(getattr(self, "_last_func_xmax", 10)))
        x_max_edit.setPlaceholderText("如 10 或 pi/2 或 2*pi")
        func_row2.addWidget(x_max_edit)
        func_row2.addSpacing(8)
        func_row2.addWidget(QLabel("点数:"))
        points_spin = QSpinBox()
        points_spin.setRange(10, 20000)
        points_spin.setValue(getattr(self, "_last_func_points", 500))
        points_spin.setSingleStep(50)
        func_row2.addWidget(points_spin)
        func_row2.addStretch()
        func_layout.addLayout(func_row2)

        func_btn_row = QHBoxLayout()
        btn_gen_func = QPushButton("从函数生成数据")
        func_btn_row.addWidget(btn_gen_func)
        func_btn_row.addStretch()
        func_layout.addLayout(func_btn_row)

        main_layout.addWidget(func_container)

        # 按钮
        btn_layout = QHBoxLayout()
        btn_parse = QPushButton("解析预览")
        btn_ok = QPushButton("确认导入")
        btn_cancel = QPushButton("取消")
        btn_layout.addWidget(btn_parse)
        btn_layout.addStretch()
        btn_layout.addWidget(btn_ok)
        btn_layout.addWidget(btn_cancel)
        main_layout.addLayout(btn_layout)

        parsed_df = {"df": None}

        def fill_xy_header():
            x_name_edit.setText("x")
            y_name_edit.setText("y")
            text_x.clear()
            text_y.clear()
            # 同步更新上次保存
            self._last_input_xname = "x"
            self._last_input_yname = "y"
            self._last_input_text_x = ""
            self._last_input_text_y = ""

        def update_preview(df, remember_input_text=True):
            # 更新预览
            preview.clear()
            preview.setRowCount(min(200, len(df)))
            preview.setColumnCount(len(df.columns))
            preview.setHorizontalHeaderLabels([str(c) for c in df.columns])
            for r in range(min(200, len(df))):
                for c in range(len(df.columns)):
                    val = df.iat[r, c]
                    preview.setItem(r, c, QTableWidgetItem(str(val)))

            # 列选择填充
            x_combo.clear(); y_combo.clear(); x2_combo.clear(); y2_combo.clear()
            for col in df.columns:
                x_combo.addItem(str(col))
                y_combo.addItem(str(col))
                x2_combo.addItem(str(col))
                y2_combo.addItem(str(col))
            # 优先选择上次使用的列名（若存在且有效），否则默认第 1/2 列
            last_sel_x = getattr(self, "_last_input_sel_xcol", None)
            last_sel_y = getattr(self, "_last_input_sel_ycol", None)
            last_sel_x2 = self.settings_state.get('x2col', '')
            last_sel_y2 = self.settings_state.get('y2col', '')
            cols_list = [str(c) for c in df.columns]
            if last_sel_x in cols_list:
                x_combo.setCurrentText(last_sel_x)
            elif len(cols_list) >= 1:
                x_combo.setCurrentIndex(0)
            if last_sel_y in cols_list:
                y_combo.setCurrentText(last_sel_y)
            elif len(cols_list) >= 2:
                y_combo.setCurrentIndex(1)
            if self.settings_state.get('enable_twinx', False):
                if last_sel_x2 in cols_list:
                    x2_combo.setCurrentText(last_sel_x2)
                elif len(cols_list) >= 2:
                    x2_combo.setCurrentIndex(1)
            if self.settings_state.get('enable_twiny', False):
                if last_sel_y2 in cols_list:
                    y2_combo.setCurrentText(last_sel_y2)
                elif len(cols_list) >= 3:
                    y2_combo.setCurrentIndex(2)

            parsed_df["df"] = df

            # 记住本次输入内容与列名，便于下次打开自动填充
            if remember_input_text:
                try:
                    self._last_input_xname = x_name_edit.text().strip() or "x"
                    self._last_input_yname = y_name_edit.text().strip() or "y"
                    self._last_input_text_x = text_x.toPlainText()
                    self._last_input_text_y = text_y.toPlainText()
                except Exception:
                    pass

            QMessageBox.information(dlg, "成功", "解析完成，可确认导入")

        def generate_x_values():
            rng_text = x_range_edit.text().strip()
            if not rng_text:
                QMessageBox.warning(dlg, "提示", "请输入起止范围，如 -10,10 或 0,2*pi")
                return
            parts = re.split(r"[\s,，]+", rng_text)
            parts = [p for p in parts if p]
            if len(parts) != 2:
                QMessageBox.warning(dlg, "提示", "范围格式应为两个数字，例如 -10,10 或 0,2*pi")
                return
            
            # 支持 pi, e 等常量的表达式解析
            allowed_funcs = {
                'sin': np.sin, 'cos': np.cos, 'tan': np.tan,
                'sinh': np.sinh, 'cosh': np.cosh, 'tanh': np.tanh,
                'exp': np.exp, 'log': np.log, 'log10': np.log10,
                'sqrt': np.sqrt, 'abs': np.abs,
                'arctan': np.arctan, 'arctan2': np.arctan2,
                'pi': np.pi, 'e': np.e
            }
            
            try:
                # 替换 ^ 为 **
                parts = [p.replace('^', '**') for p in parts]
                
                # 安全解析表达式
                import ast
                for part_expr in parts:
                    tree = ast.parse(part_expr, mode='eval')
                    # 检查合法性
                    for node in ast.walk(tree):
                        if isinstance(node, ast.Name):
                            if node.id not in allowed_funcs:
                                raise ValueError(f"不支持的符号: {node.id}")
                        elif isinstance(node, ast.Call):
                            if not isinstance(node.func, ast.Name):
                                raise ValueError("函数调用不合法")
                            if node.func.id not in allowed_funcs:
                                raise ValueError(f"不支持的函数: {node.func.id}")
                
                # 计算范围值
                env = {k: v for k, v in allowed_funcs.items() if callable(v)}
                env.update({'pi': np.pi, 'e': np.e})
                start = float(eval(parts[0], {"__builtins__": {}}, env))
                end = float(eval(parts[1], {"__builtins__": {}}, env))
                
            except Exception as e:
                QMessageBox.warning(dlg, "提示", f"范围表达式无效：{e}\n支持: + - * / ^, sin cos tan exp log sqrt abs, 常量: pi e")
                return
            
            n = x_points_spin.value()
            if n < 2:
                QMessageBox.warning(dlg, "提示", "点数需至少为 2")
                return
            xs = np.linspace(start, end, n)
            text_x.setPlainText("\n".join(f"{v:.6g}" for v in xs))

            # 记录上次自动生成参数
            try:
                self._last_auto_x_range = rng_text
                self._last_auto_x_points = n
                self._last_input_text_x = text_x.toPlainText()
            except Exception:
                pass

            QMessageBox.information(dlg, "成功", f"已生成 X {n} 个点，从 {start:.6g} 到 {end:.6g}")

        def parse_text():
            # 将每侧输入自动打散为“一列”：按逗号/空格/分号/制表符/中文逗号分隔并去空
            def flatten_to_one_column(text: str):
                tokens = []
                for ln in text.splitlines():
                    ln = ln.strip()
                    if not ln:
                        continue
                    parts = re.split(r"[\,\t;；，\s]+", ln)
                    for p in parts:
                        p = p.strip()
                        if p:
                            tokens.append(p)
                return tokens

            xs_raw = flatten_to_one_column(text_x.toPlainText())
            ys_raw = flatten_to_one_column(text_y.toPlainText())

            if not xs_raw or not ys_raw:
                QMessageBox.warning(dlg, "提示", "请在 X/Y 输入框分别输入数据（系统会自动拆分为一列）")
                return
            if len(xs_raw) != len(ys_raw):
                QMessageBox.warning(dlg, "提示", f"X 与 Y 数量不一致：X={len(xs_raw)}, Y={len(ys_raw)}。请检查两侧数据是否对应")
                return
            try:
                xs_val = pd.to_numeric(xs_raw, errors='raise')
                ys_val = pd.to_numeric(ys_raw, errors='raise')
            except Exception as e:
                QMessageBox.critical(dlg, "解析失败", f"存在无法转换为数字的值：{e}")
                return

            xname = x_name_edit.text().strip() or "x"
            yname = y_name_edit.text().strip() or "y"
            df = pd.DataFrame({xname: xs_val, yname: ys_val})
            update_preview(df, remember_input_text=True)

        def generate_from_function():
            expr = func_edit.text().strip()
            if not expr:
                QMessageBox.warning(dlg, "提示", "请输入函数表达式 f(x)")
                return
            expr = expr.replace("^", "**")
            
            # 支持 pi, e 等常量和表达式
            allowed_funcs = {
                'sin': np.sin, 'cos': np.cos, 'tan': np.tan,
                'sinh': np.sinh, 'cosh': np.cosh, 'tanh': np.tanh,
                'exp': np.exp, 'log': np.log, 'log10': np.log10,
                'sqrt': np.sqrt, 'abs': np.abs,
                'arctan': np.arctan, 'arctan2': np.arctan2,
                'pi': np.pi, 'e': np.e
            }
            
            try:
                # 解析 x 最小值
                x_min_text = x_min_edit.text().strip()
                if x_min_text:
                    x_min_text = x_min_text.replace("^", "**")
                    env = {k: v for k, v in allowed_funcs.items() if callable(v)}
                    env.update({'pi': np.pi, 'e': np.e})
                    x_min = float(eval(x_min_text, {"__builtins__": {}}, env))
                else:
                    x_min = -10.0
                
                # 解析 x 最大值
                x_max_text = x_max_edit.text().strip()
                if x_max_text:
                    x_max_text = x_max_text.replace("^", "**")
                    env = {k: v for k, v in allowed_funcs.items() if callable(v)}
                    env.update({'pi': np.pi, 'e': np.e})
                    x_max = float(eval(x_max_text, {"__builtins__": {}}, env))
                else:
                    x_max = 10.0
                    
            except Exception as e:
                QMessageBox.warning(dlg, "提示", f"x 范围表达式无效：{e}\n支持: + - * / ^, sin cos tan exp log sqrt abs, 常量: pi e")
                return
            
            if x_max <= x_min:
                QMessageBox.warning(dlg, "提示", "x 最大必须大于 x 最小")
                return
            points = points_spin.value()

            # 安全表达式解析（仅允许有限节点和函数）
            import ast
            allowed_names = set(allowed_funcs.keys()) | {"x"}
            allowed_nodes = (
                ast.Expression, ast.BinOp, ast.UnaryOp, ast.Call, ast.Load,
                ast.Add, ast.Sub, ast.Mult, ast.Div, ast.Pow, ast.Mod,
                ast.USub, ast.UAdd,
                ast.Constant, ast.Name
            )

            try:
                tree = ast.parse(expr, mode='eval')
                for node in ast.walk(tree):
                    if not isinstance(node, allowed_nodes):
                        raise ValueError("包含不支持的语法元素")
                    if isinstance(node, ast.Name):
                        if node.id not in allowed_names:
                            raise ValueError(f"不支持的符号: {node.id}")
                    if isinstance(node, ast.Call):
                        if not isinstance(node.func, ast.Name):
                            raise ValueError("函数调用不合法")
                        if node.func.id not in allowed_funcs:
                            raise ValueError(f"不支持的函数: {node.func.id}")
                code_obj = compile(tree, "<expr>", "eval")
            except Exception as e:
                QMessageBox.critical(dlg, "解析失败", f"函数表达式无效：{e}")
                return

            x_vals = np.linspace(x_min, x_max, points)
            env = {k: v for k, v in allowed_funcs.items() if callable(v)}
            env.update({'pi': np.pi, 'e': np.e, 'x': x_vals})
            try:
                y_vals = eval(code_obj, {"__builtins__": {}}, env)
                y_vals = np.array(y_vals, dtype=float)
            except Exception as e:
                QMessageBox.critical(dlg, "计算失败", f"函数计算出错：{e}")
                return

            if not np.all(np.isfinite(y_vals)):
                QMessageBox.warning(dlg, "提示", "生成的数据包含非有限值（NaN/Inf）。请检查函数或范围。")
                return
            if y_vals.shape != x_vals.shape:
                QMessageBox.warning(dlg, "提示", "函数返回的数据形状不匹配 X 范围。")
                return

            xname = x_name_edit.text().strip() or "x"
            yname = y_name_edit.text().strip() or "y"
            df = pd.DataFrame({xname: x_vals, yname: y_vals})

            # 记住函数输入参数
            try:
                self._last_func_expr = expr
                self._last_func_xmin = x_min
                self._last_func_xmax = x_max
                self._last_func_points = points
            except Exception:
                pass

            update_preview(df, remember_input_text=False)
            # update_preview 已完成预览、列选择与提示
            return

        def accept_import():
            df = parsed_df.get("df")
            if df is None:
                QMessageBox.warning(dlg, "提示", "请先解析再导入")
                return
            xcol = x_combo.currentText()
            ycol = y_combo.currentText()
            x2col = x2_combo.currentText() if self.settings_state.get('enable_twinx', False) else ''
            y2col = y2_combo.currentText() if self.settings_state.get('enable_twiny', False) else ''
            if not xcol or not ycol:
                QMessageBox.warning(dlg, "提示", "请选择 X/Y 列")
                return
            if self.settings_state.get('enable_twinx', False) and not x2col:
                QMessageBox.warning(dlg, "提示", "请选择顶部 X 列")
                return
            if self.settings_state.get('enable_twiny', False) and not y2col:
                QMessageBox.warning(dlg, "提示", "请选择右侧 Y 列")
                return

            self.tasks.cancel_all()
            # 清除示例占位
            if getattr(self, "placeholder_active", False):
                self.loaded_files.clear()
                self.history.reset(self.loaded_files)
                try:
                    self.combo_x.clear(); self.combo_y.clear()
                except Exception:
                    pass
                self.placeholder_active = False

            # 记录数据
            self.input_counter += 1
            tag = f"input_data_{self.input_counter}"
            self.loaded_files.append((tag, df))
            self.history.reset(self.loaded_files)

            # 记住列选择，便于下次打开默认选择相同列
            try:
                self._last_input_sel_xcol = xcol
                self._last_input_sel_ycol = ycol
                self.settings_state['x2col'] = x2col
                self.settings_state['y2col'] = y2col
            except Exception:
                pass

            # 更新下拉列
            try:
                if xcol not in [self.combo_x.itemText(i) for i in range(self.combo_x.count())]:
                    self.combo_x.addItem(xcol)
                if ycol not in [self.combo_y.itemText(i) for i in range(self.combo_y.count())]:
                    self.combo_y.addItem(ycol)
                self.combo_x.setCurrentText(xcol)
                self.combo_y.setCurrentText(ycol)
            except Exception:
                pass

            dlg.accept()
            self.replot_all()
            self.statusBar().showMessage(f"已导入手工输入数据 ({xcol} vs {ycol})", 4000)

        btn_parse.clicked.connect(parse_text)
        btn_ok.clicked.connect(accept_import)
        btn_cancel.clicked.connect(dlg.reject)
        btn_fill_xy.clicked.connect(fill_xy_header)
        btn_gen_func.clicked.connect(generate_from_function)
        btn_gen_x.clicked.connect(generate_x_values)

        def on_toggle(checked: bool):
            func_container.setVisible(checked)
            func_toggle.setText("收起函数生成" if checked else "展开函数生成 (可选)")

        func_toggle.toggled.connect(on_toggle)

        dlg.exec()

    def open_legend_config(self):
        """打开图例配置对话框"""
        dlg = QDialog(self)
        dlg.setWindowTitle("图例设置")
        dlg.resize(500, 600)
        layout = QVBoxLayout(dlg)
        layout.setSpacing(12)

        # 显示图例
        show_layout = QHBoxLayout()
        show_layout.addWidget(QLabel("显示图例:"))
        show_check = QCheckBox()
        show_check.setChecked(self.legend_config["show"])
        show_layout.addWidget(show_check)
        show_layout.addStretch()
        layout.addLayout(show_layout)

        # 位置选择
        loc_layout = QHBoxLayout()
        loc_layout.addWidget(QLabel("位置:"))
        loc_combo = QComboBox()
        locations = ["upper left", "upper right", "lower left", "lower right", 
                    "upper center", "lower center", "center left", "center right", "center"]
        loc_combo.addItems(locations)
        loc_combo.setCurrentText(self.legend_config["loc"])
        loc_layout.addWidget(loc_combo)
        loc_layout.addStretch()
        layout.addLayout(loc_layout)

        # 字体大小
        font_layout = QHBoxLayout()
        font_layout.addWidget(QLabel("字体大小:"))
        font_spin = QSpinBox()
        font_spin.setRange(6, 24)
        font_spin.setValue(self.legend_config["fontsize"])
        font_layout.addWidget(font_spin)
        font_layout.addStretch()
        layout.addLayout(font_layout)

        # 是否显示边框
        frame_layout = QHBoxLayout()
        frame_layout.addWidget(QLabel("显示边框:"))
        frame_check = QCheckBox()
        frame_check.setChecked(self.legend_config["frameon"])
        frame_layout.addWidget(frame_check)
        frame_layout.addStretch()
        layout.addLayout(frame_layout)

        # 透明度
        alpha_layout = QHBoxLayout()
        alpha_layout.addWidget(QLabel("透明度 (0-1):"))
        alpha_spin = QDoubleSpinBox()
        alpha_spin.setRange(0.0, 1.0)
        alpha_spin.setSingleStep(0.1)
        alpha_spin.setValue(self.legend_config["framealpha"])
        alpha_layout.addWidget(alpha_spin)
        alpha_layout.addStretch()
        layout.addLayout(alpha_layout)

        # 曲线标签编辑区
        layout.addWidget(QLabel("编辑曲线标签:"))
        label_table = QTableWidget()
        label_table.setColumnCount(2)
        label_table.setHorizontalHeaderLabels(["文件/数据", "图例标签"])
        label_table.setRowCount(len(self.loaded_files))
        
        # 初始化标签编辑表格
        label_edits = []
        for i, (file_path, df) in enumerate(self.loaded_files):
            file_name = os.path.splitext(os.path.basename(file_path))[0]
            label_table.setItem(i, 0, QTableWidgetItem(file_name))
            
            # 从缓存中读取或使用默认标签
            if not hasattr(self, "_legend_labels"):
                self._legend_labels = {}
            
            current_label = self._legend_labels.get(file_name, file_name)
            label_edit = QLineEdit(current_label)
            label_edits.append((file_name, label_edit))
            label_table.setCellWidget(i, 1, label_edit)
        
        label_table.resizeColumnsToContents()
        label_table.setMaximumHeight(200)
        layout.addWidget(label_table)

        # 按钮
        btn_layout = QHBoxLayout()
        btn_apply = QPushButton("应用")
        btn_cancel = QPushButton("取消")
        btn_layout.addStretch()
        btn_layout.addWidget(btn_apply)
        btn_layout.addWidget(btn_cancel)
        layout.addLayout(btn_layout)

        def apply_config():
            self.legend_config["show"] = show_check.isChecked()
            self.legend_config["loc"] = loc_combo.currentText()
            self.legend_config["fontsize"] = font_spin.value()
            self.legend_config["frameon"] = frame_check.isChecked()
            self.legend_config["framealpha"] = alpha_spin.value()
            
            # 保存标签编辑
            for file_name, label_edit in label_edits:
                self._legend_labels[file_name] = label_edit.text().strip() or file_name
            
            dlg.accept()
            # 刷新图例
            self.replot_all()

        btn_apply.clicked.connect(apply_config)
        btn_cancel.clicked.connect(dlg.reject)

        dlg.exec()

    def open_delete_line_dialog(self):
        """打开删除线条对话框：选择并删除一条或多条已绘制的线"""
        if not self.loaded_files:
            QMessageBox.warning(self, "提示", "没有加载任何数据")
            return

        dlg = QDialog(self)
        dlg.setWindowTitle("删除线条")
        dlg.resize(450, 350)
        layout = QVBoxLayout(dlg)
        layout.setSpacing(12)

        # 说明信息
        layout.addWidget(QLabel("选择要删除的线条（可多选）："))

        # 创建文件列表（支持多选）
        file_list = QListWidget()
        file_list.setSelectionMode(QAbstractItemView.SelectionMode.MultiSelection)  # 启用多选模式
        for i, (file_path, df) in enumerate(self.loaded_files):
            file_name = os.path.basename(file_path)
            file_item = QListWidgetItem(file_name)
            file_item.setData(Qt.UserRole, i)  # 存储索引
            file_list.addItem(file_item)
        
        layout.addWidget(file_list)

        # 按钮
        btn_layout = QHBoxLayout()
        btn_delete = QPushButton("删除选中线")
        btn_cancel = QPushButton("取消")
        btn_layout.addStretch()
        btn_layout.addWidget(btn_delete)
        btn_layout.addWidget(btn_cancel)
        layout.addLayout(btn_layout)

        def delete_line():
            selected_items = file_list.selectedItems()
            if not selected_items:
                QMessageBox.warning(dlg, "提示", "请先选择要删除的线条")
                return
            
            # 获取所有选中的索引和名称
            indices_to_delete = []
            file_names = []
            for item in selected_items:
                file_index = item.data(Qt.UserRole)
                indices_to_delete.append(file_index)
                file_names.append(item.text())
            
            # 确认弹框
            delete_count = len(indices_to_delete)
            confirm_text = f"确定要删除以下 {delete_count} 条线条吗？\n\n" + "\n".join(file_names)
            ret = QMessageBox.question(
                dlg, 
                "确认删除", 
                confirm_text,
                QMessageBox.Yes | QMessageBox.No
            )
            
            if ret == QMessageBox.Yes:
                self._commit_file_deletions(indices_to_delete)
                
                # 如果还有数据，重新绘图；否则清空
                if self.loaded_files:
                    # 删除文件后更新 combo 列表
                    self._update_combo_columns()
                    xcol = self.combo_x.currentText()
                    ycol = self.combo_y.currentText()
                    if xcol and ycol:
                        self.replot_all()
                    self.statusBar().showMessage(f"已删除 {delete_count} 条线条")
                else:
                    self.ax.clear()
                    self.canvas.draw()
                    try:
                        self.combo_x.clear()
                        self.combo_y.clear()
                    except Exception:
                        pass
                    self.statusBar().showMessage("已删除所有线条")
                
                dlg.accept()

        btn_delete.clicked.connect(delete_line)
        btn_cancel.clicked.connect(dlg.reject)

        dlg.exec()

    def open_publish_dialog(self):
        """打开发布绘图对话框：用于选择数据并微调样式后导出高质量图片"""
        if not self.loaded_files:
            QMessageBox.warning(self, "提示", "请先加载数据")
            return

        xcol = self.combo_x.currentText()
        ycol = self.combo_y.currentText()
        if not xcol or not ycol:
            QMessageBox.warning(self, "提示", "请选择 X 列和 Y 列")
            return

        # 备份主界面的 settings_state（用于对话框结束后恢复）
        prev_settings_state = getattr(self, 'settings_state', None)

        # 出版绘图默认设置（每次打开对话框都使用独立配置，不影响主界面）
        self.settings_state = {
            'selected_files': {os.path.basename(self.loaded_files[0][0])} if self.loaded_files else set(),
            'linestyle': '-',
            'marker': 'o',
            'markersize': 6.0,
            'linewidth': 2.0,
            'color': '#1f77b4',
            'color_scheme_type': '固定颜色',
            'colormap': 'viridis',
            'xlabel': 'XLabel',
            'ylabel': 'YLabel',
            'fontsize': 28,
            'fontfamily': 'Helvetica',
            'use_latex': False,
            'xlabel_pad': 3,
            'ylabel_pad': 0,
            'tick_dir': 'in',
            'tick_len': 8.0,
            'tick_wid': 2.0,
            'tick_label_size': 24,
            'tick_axis': 'both',
            'minor_ticks': False,
            'minor_x_interval': '',
            'minor_y_interval': '',
            'per_series_style': {},
            'frame_width': 2.0,
            'radian_mode': (self.x_unit_mode == 'radian'),
            'legend_fontsize': 26,
            'legend_loc': 'lower right',
            'legend_labels': {},
            'major_x_interval': '',
            'major_y_interval': '',
            'x_min': '',
            'x_max': '',
            'y_min': '',
            'y_max': '',
            'fig_w': 7.5,
            'fig_h': 7.0,
            'enable_twinx': False,
            'enable_twiny': False,
            'x2col': '',
            'y2col': '',
            'x2label': '',
            'y2label': '',
            'x2_min': '',
            'x2_max': '',
            'y2_min': '',
            'y2_max': '',
        }

        dlg = QDialog(self)
        dlg.setWindowTitle("出版绘图")
        # 对话框尺寸稍小一些
        try:
            base_size = min(self.width(), self.height())
            base_size = max(base_size, 550)
            dlg.resize(base_size, base_size)
            dlg.setMinimumSize(550, 550)
        except Exception:
            dlg.resize(700, 650)

        root = QVBoxLayout(dlg)

        # 顶部：设置按钮
        btn_basic = QPushButton("设置")
        btn_basic.setStyleSheet("font-weight:600;")
        root.addWidget(btn_basic)

        # 预览画布容器（用来装 canvas）
        preview_container = QWidget()
        preview_container_layout = QVBoxLayout(preview_container)
        preview_container_layout.setContentsMargins(0, 0, 0, 0)
        preview_container_layout.setAlignment(Qt.AlignCenter)  # 居中对齐
        root.addWidget(preview_container)
        
        # 初始化预览 Figure 和 Canvas
        preview_fig = None
        preview_ax = None
        preview_canvas = None

        # 打开设置对话框
        def open_basic_settings():
            settings_dlg = QDialog(dlg)
            settings_dlg.setWindowTitle("设置")
            # 设置窗口大小：为出版绘图窗口大小的 2/3
            try:
                w = max(dlg.width(), 550)
                h = max(dlg.height(), 550)
                settings_dlg.resize(int(w * 2 / 3), int(h * 2 / 3))
                settings_dlg.setMinimumSize(500, 450)
            except Exception:
                settings_dlg.resize(700, 500)
            main_layout = QVBoxLayout(settings_dlg)
            
            # 添加滚动区域
            from PySide6.QtWidgets import QScrollArea
            from PySide6.QtCore import Qt
            scroll = QScrollArea()
            scroll.setWidgetResizable(True)
            scroll.setHorizontalScrollBarPolicy(Qt.ScrollBarAlwaysOff)
            scroll_content = QWidget()
            s_layout = QVBoxLayout(scroll_content)
            scroll.setWidget(scroll_content)
            main_layout.addWidget(scroll)

            # 曲线选择 + 全选
            from PySide6.QtCore import Qt
            select_row = QHBoxLayout()
            select_row.addWidget(QLabel("选择曲线:"))
            select_row.addSpacing(12)
            select_all_cb = QCheckBox("全选")
            select_all_cb.setTristate(False)  # 改为两态，避免中间状态混淆
            select_row.addWidget(select_all_cb)
            select_row.addStretch()
            s_layout.addLayout(select_row)

            files_list = QListWidget()
            # 使用多选模式，但通过自定义逻辑控制单选/多选行为
            files_list.setSelectionMode(QListWidget.MultiSelection)
            
            # 保存最后一次点击的项索引，用于 Shift+点击范围选择
            last_clicked_index = [-1]  # 使用列表来避免 nonlocal 问题
            
            # 自定义 mouse press event 以实现 Shift+点击多选
            original_mousePressEvent = files_list.mousePressEvent
            def custom_mousePressEvent(event):
                from PySide6.QtCore import Qt
                
                # 获取点击的项
                item = files_list.itemAt(event.pos())
                if item is None:
                    original_mousePressEvent(event)
                    return
                
                current_index = files_list.row(item)
                modifiers = event.modifiers()
                
                if modifiers & Qt.ShiftModifier and last_clicked_index[0] >= 0:
                    # Shift+点击：范围选择
                    # 计算范围
                    start_idx = min(last_clicked_index[0], current_index)
                    end_idx = max(last_clicked_index[0], current_index)
                    
                    # 选中范围内所有项
                    for i in range(start_idx, end_idx + 1):
                        files_list.item(i).setSelected(True)
                    
                    self.settings_state['selected_files'] = {it.text() for it in files_list.selectedItems()}
                    # 更新最后点击索引
                    last_clicked_index[0] = current_index
                else:
                    # 普通点击：单选（清除其他选择）
                    files_list.clearSelection()
                    item.setSelected(True)
                    last_clicked_index[0] = current_index
                    self.settings_state['selected_files'] = {item.text()}
                
                # 触发信号以更新 UI
                files_list.itemSelectionChanged.emit()
                event.accept()
            
            files_list.mousePressEvent = custom_mousePressEvent
            
            for idx, (file_path, _df) in enumerate(self.loaded_files):
                basename = os.path.basename(file_path)
                it = QListWidgetItem(basename)
                files_list.addItem(it)
                # 如果没有任何选择，默认选择第一个文件
                if not self.settings_state['selected_files'] and idx == 0:
                    it.setSelected(True)
                    self.settings_state['selected_files'].add(basename)
                    last_clicked_index[0] = 0
                else:
                    it.setSelected(basename in self.settings_state['selected_files'])
            # 如果仍然没有任何被选中的项，强制选中第一项
            try:
                if files_list.count() > 0 and len(files_list.selectedItems()) == 0:
                    files_list.item(0).setSelected(True)
                    first_name = files_list.item(0).text()
                    self.settings_state['selected_files'] = {first_name}
                    last_clicked_index[0] = 0
            except Exception:
                pass
            
            # 提前初始化这些变量，供后续函数使用
            legend_label_edits = []  # 保存编辑框引用
            target_combo = None
            legend_table = None
            last_edit = None

            # 全选/全不选行为与勾选状态联动
            def update_select_all_checkbox():
                total = files_list.count()
                selected = len(files_list.selectedItems())
                # 同时更新 settings_state
                self.settings_state['selected_files'] = {it.text() for it in files_list.selectedItems()}
                # 不触发信号，避免循环
                select_all_cb.blockSignals(True)
                if selected == total and total > 0:
                    select_all_cb.setChecked(True)
                else:
                    select_all_cb.setChecked(False)
                select_all_cb.blockSignals(False)
                
                # 只有当 target_combo 和 legend_table 都已初始化后，才执行动态更新
                if target_combo is None or legend_table is None:
                    return
                
                # 动态更新"自定义目标"下拉框
                def update_target_combo():
                    current_sel = target_combo.currentText()
                    target_combo.blockSignals(True)
                    target_combo.clear()
                    new_target_names = [os.path.basename(f[0]) for f in self.loaded_files if os.path.basename(f[0]) in self.settings_state['selected_files']]
                    if not new_target_names:
                        new_target_names = [os.path.basename(f[0]) for f in self.loaded_files]
                    target_combo.addItems(new_target_names)
                    # 尽量保持之前的选择
                    if current_sel in new_target_names:
                        target_combo.setCurrentText(current_sel)
                    target_combo.blockSignals(False)
                
                # 动态更新"自定义图例标签"表格
                def update_legend_table():
                    selected_files_list = [f for f in self.loaded_files if os.path.basename(f[0]) in self.settings_state['selected_files']]
                    legend_table.setRowCount(len(selected_files_list))
                    legend_label_edits.clear()
                    for i, (file_path, df) in enumerate(selected_files_list):
                        file_name = os.path.basename(file_path)
                        legend_table.setItem(i, 0, QTableWidgetItem(file_name))
                        # 获取当前标签（可能是之前自定义的）
                        current_label = self.settings_state['legend_labels'].get(file_name, '')
                        label_edit = QLineEdit(current_label)
                        label_edit.setPlaceholderText(os.path.splitext(file_name)[0])
                        legend_label_edits.append((file_name, label_edit))
                        legend_table.setCellWidget(i, 1, label_edit)
                        # 添加焦点事件
                        def make_focus_handler(edit):
                            def on_focus():
                                nonlocal last_edit
                                last_edit = edit
                            return on_focus
                        focus_handler = make_focus_handler(label_edit)
                        label_edit.mousePressEvent = lambda e, h=focus_handler, le=label_edit: (h(), QLineEdit.mousePressEvent(le, e))
                    legend_table.resizeColumnsToContents()
                    # 动态调整表格高度
                    row_count = legend_table.rowCount()
                    header_height = legend_table.horizontalHeader().height()
                    estimated_height = header_height + (row_count * 40)
                    legend_table.setMaximumHeight(max(estimated_height, 400))
                
                # 执行更新
                try:
                    update_target_combo()
                except Exception:
                    pass
                try:
                    update_legend_table()
                except Exception:
                    pass

            def on_select_all_state(checked:bool):
                total = files_list.count()
                # 不触发信号，避免循环
                files_list.blockSignals(True)
                if checked:
                    for i in range(total):
                        files_list.item(i).setSelected(True)
                    self.settings_state['selected_files'] = {files_list.item(i).text() for i in range(total)}
                else:
                    for i in range(total):
                        files_list.item(i).setSelected(False)
                    self.settings_state['selected_files'] = set()
                files_list.blockSignals(False)
                # 手动触发动态更新（因为 blockSignals 阻止了 itemSelectionChanged 信号）
                update_select_all_checkbox()

            select_all_cb.stateChanged.connect(on_select_all_state)
            # 暂时不连接 itemSelectionChanged，待 target_combo 和 legend_table 创建后再连接
            # files_list.itemSelectionChanged.connect(update_select_all_checkbox)
            update_select_all_checkbox()  # 初始化状态
            try:
                files_list.setMaximumHeight(120)
            except Exception:
                pass
            s_layout.addWidget(files_list)

            # 全局设置：字体、LaTeX、图尺寸
            from PySide6.QtWidgets import QGroupBox
            group_global = QGroupBox("全局")
            global_layout = QVBoxLayout()
            
            # 字体（仅保留 Helvetica / Times New Roman）
            font_row = QHBoxLayout()
            font_row.addWidget(QLabel("字体:"))
            from PySide6.QtGui import QFontDatabase
            font_combo = QComboBox()
            common_fonts = ["Helvetica", "Times New Roman"]
            available_fonts = [f for f in common_fonts if f in QFontDatabase.families()]
            if not available_fonts:
                available_fonts = common_fonts
            font_combo.addItems(available_fonts)
            if self.settings_state['fontfamily'] and self.settings_state['fontfamily'] in available_fonts:
                font_combo.setCurrentText(self.settings_state['fontfamily'])
            font_row.addWidget(font_combo)
            font_row.addStretch()
            global_layout.addLayout(font_row)

            # 双轴开关（互斥）
            twin_row = QHBoxLayout()
            twin_x_cb = QCheckBox("双 X 轴(顶部)")
            twin_y_cb = QCheckBox("双 Y 轴(右侧)")
            twin_x_cb.setChecked(self.settings_state.get('enable_twinx', False))
            twin_y_cb.setChecked(self.settings_state.get('enable_twiny', False))
            
            # 添加互斥逻辑
            def on_twin_x_changed(checked):
                if checked:
                    twin_y_cb.setChecked(False)
            
            def on_twin_y_changed(checked):
                if checked:
                    twin_x_cb.setChecked(False)
            
            twin_x_cb.toggled.connect(on_twin_x_changed)
            twin_y_cb.toggled.connect(on_twin_y_changed)
            
            twin_row.addWidget(twin_x_cb)
            twin_row.addSpacing(12)
            twin_row.addWidget(twin_y_cb)
            twin_row.addStretch()
            global_layout.addLayout(twin_row)

            # 图尺寸（英寸），与字体同组，放在“选择曲线”之后
            size_row = QHBoxLayout()
            size_row.addWidget(QLabel("图宽(in):"))
            fig_w_spin = QDoubleSpinBox()
            fig_w_spin.setRange(2.0, 20.0)
            fig_w_spin.setSingleStep(0.5)
            fig_w_spin.setValue(self.settings_state.get('fig_w', 8.0))
            fig_w_spin.setMaximumWidth(90)
            size_row.addWidget(fig_w_spin)
            size_row.addSpacing(8)
            size_row.addWidget(QLabel("图高(in):"))
            fig_h_spin = QDoubleSpinBox()
            fig_h_spin.setRange(2.0, 20.0)
            fig_h_spin.setSingleStep(0.5)
            fig_h_spin.setValue(self.settings_state.get('fig_h', 8.0))
            fig_h_spin.setMaximumWidth(90)
            size_row.addWidget(fig_h_spin)
            size_row.addStretch()
            global_layout.addLayout(size_row)
            
            group_global.setLayout(global_layout)
            s_layout.addWidget(group_global)

            # 线条
            from PySide6.QtWidgets import QGroupBox
            # 线条
            group_line = QGroupBox("线条")
            line_layout = QVBoxLayout()
            
            # 第一行：线型、颜色、渐变色选项
            line_row1 = QHBoxLayout()
            line_row1.addWidget(QLabel("线型:"))
            ls_combo = QComboBox()
            ls_combo.addItems(['无', '-', '--', '-.', ':'])
            ls_combo.setCurrentText(self.settings_state['linestyle'])
            ls_combo.setMinimumWidth(80)
            line_row1.addWidget(ls_combo)
            line_row1.addSpacing(12)
            line_row1.addWidget(QLabel("颜色:"))
            
            # 颜色选择模式：固定颜色 vs 渐变色
            color_mode_combo = QComboBox()
            color_mode_combo.addItems(['固定颜色', '渐变色'])
            color_mode_combo.setCurrentText(self.settings_state['color_scheme_type'])
            color_mode_combo.setMaximumWidth(100)
            line_row1.addWidget(color_mode_combo)
            
            # 固定颜色选择
            color_combo = QComboBox()
            from PySide6.QtGui import QPixmap, QIcon, QColor
            palette_hex = [
                "#1f77b4", "#ff7f0e", "#2ca02c", "#d62728", "#9467bd",
                "#8c564b", "#e377c2", "#7f7f7f", "#bcbd22", "#17becf"
            ]
            for hx in palette_hex:
                pix = QPixmap(24, 16)
                pix.fill(QColor(hx))
                color_combo.addItem(QIcon(pix), "", hx)
            # 设置当前颜色索引
            cur_idx = 0
            if self.settings_state['color_scheme_type'] == '固定颜色':
                for i, hx in enumerate(palette_hex):
                    if hx.lower() == self.settings_state['color'].lower():
                        cur_idx = i
                        break
            color_combo.setCurrentIndex(cur_idx)
            color_combo.setMaximumWidth(80)
            line_row1.addWidget(color_combo)
            
            # 渐变色选择
            colormap_combo = QComboBox()
            colormaps = ['viridis', 'plasma', 'inferno', 'magma', 'cividis',
                        'Blues', 'Greens', 'Reds', 'Purples', 'Oranges',
                        'cool', 'hot', 'spring', 'summer', 'autumn', 'winter',
                        'gray', 'bone', 'pink', 'copper']
            colormap_combo.addItems(colormaps)
            colormap_combo.setCurrentText(self.settings_state.get('colormap', 'viridis'))
            colormap_combo.setMaximumWidth(120)
            colormap_combo.setVisible(self.settings_state['color_scheme_type'] == '渐变色')
            line_row1.addWidget(colormap_combo)
            
            # 颜色模式切换事件
            def on_color_mode_changed(mode):
                color_combo.setVisible(mode == '固定颜色')
                colormap_combo.setVisible(mode == '渐变色')
                self.settings_state['color_scheme_type'] = mode
            
            color_mode_combo.currentTextChanged.connect(on_color_mode_changed)
            
            line_row1.addStretch()
            line_layout.addLayout(line_row1)
            
            # 第三行：Marker、Marker大小
            line_row3 = QHBoxLayout()
            line_row3.addWidget(QLabel("Marker:"))
            marker_combo = QComboBox()
            marker_combo.addItems(['无', '.', ',', 'o', 'v', '^', '<', '>', 's', 'p', '*', 'h', 'H', '+', 'x', 'D', 'd', '|', '_'])
            marker_combo.setCurrentText(self.settings_state['marker'] if self.settings_state['marker'] else '无')
            line_row3.addWidget(marker_combo)
            line_row3.addSpacing(12)
            line_row3.addWidget(QLabel("Marker大小:"))
            ms_spin = QDoubleSpinBox()
            ms_spin.setRange(1.0, 20.0)
            ms_spin.setSingleStep(0.5)
            ms_spin.setValue(self.settings_state['markersize'])
            line_row3.addWidget(ms_spin)
            line_row3.addStretch()
            line_layout.addLayout(line_row3)
            
            # 第四行：线宽、边框宽度
            line_row4 = QHBoxLayout()
            line_row4.addWidget(QLabel("线宽:"))
            lw_spin = QDoubleSpinBox()
            lw_spin.setRange(0.5, 8.0)
            lw_spin.setValue(self.settings_state['linewidth'])
            lw_spin.setSingleStep(0.5)
            line_row4.addWidget(lw_spin)
            line_row4.addSpacing(12)
            line_row4.addWidget(QLabel("边框宽度:"))
            frame_spin = QDoubleSpinBox()
            frame_spin.setRange(0.5, 8.0)
            frame_spin.setValue(self.settings_state['frame_width'])
            frame_spin.setSingleStep(0.5)
            line_row4.addWidget(frame_spin)
            line_row4.addStretch()
            line_layout.addLayout(line_row4)

            # 单曲线自定义（选一条曲线后保存样式）
            single_row = QHBoxLayout()
            single_row.addWidget(QLabel("自定义目标:"))
            target_combo = QComboBox()
            target_names = [os.path.basename(f[0]) for f in self.loaded_files if os.path.basename(f[0]) in self.settings_state['selected_files']]
            if not target_names:
                target_names = [os.path.basename(f[0]) for f in self.loaded_files]
            target_combo.addItems(target_names)
            single_row.addWidget(target_combo)
            single_row.addSpacing(12)
            btn_save_series = QPushButton("保存到该曲线")
            single_row.addWidget(btn_save_series)
            single_row.addStretch()
            line_layout.addLayout(single_row)

            def save_series_style():
                name = target_combo.currentText()
                # 使用当前“线条”中的设置作为该曲线样式
                color_hex = color_combo.currentData() or ''
                ls_text = ls_combo.currentText(); ls_value = '' if ls_text == '无' else ls_text
                mk_text = marker_combo.currentText(); mk_value = '' if mk_text == '无' else mk_text
                if 'per_series_style' not in self.settings_state:
                    self.settings_state['per_series_style'] = {}
                self.settings_state['per_series_style'][name] = {
                    'color': color_hex,
                    'linestyle': ls_value,
                    'linewidth': lw_spin.value(),
                    'marker': mk_value,
                    'markersize': ms_spin.value(),
                }
                # 即时刷新预览
                refresh_preview()
            btn_save_series.clicked.connect(save_series_style)
            
            group_line.setLayout(line_layout)
            s_layout.addWidget(group_line)

            # 坐标与标题设置
            group_axis = QGroupBox("标签")
            axis_layout = QVBoxLayout()
            # X轴标签单独一行
            axis_row1 = QHBoxLayout()
            axis_row1.addWidget(QLabel("X轴标签:"))
            xlabel_edit = QLineEdit(self.settings_state['xlabel'])
            axis_row1.addWidget(xlabel_edit)
            axis_layout.addLayout(axis_row1)
            
            # Y轴标签单独一行
            axis_row2 = QHBoxLayout()
            axis_row2.addWidget(QLabel("Y轴标签:"))
            ylabel_edit = QLineEdit(self.settings_state['ylabel'])
            axis_row2.addWidget(ylabel_edit)
            axis_layout.addLayout(axis_row2)
            
            # 双轴标签（根据开关显示/隐藏）
            axis_row2b = QHBoxLayout()
            x2label_lbl = QLabel("顶部X轴标签:")
            axis_row2b.addWidget(x2label_lbl)
            x2label_edit = QLineEdit(self.settings_state.get('x2label', ''))
            axis_row2b.addWidget(x2label_edit)
            axis_layout.addLayout(axis_row2b)
            
            axis_row2c = QHBoxLayout()
            y2label_lbl = QLabel("右侧Y轴标签:")
            axis_row2c.addWidget(y2label_lbl)
            y2label_edit = QLineEdit(self.settings_state.get('y2label', ''))
            axis_row2c.addWidget(y2label_edit)
            axis_layout.addLayout(axis_row2c)
            
            # 默认隐藏双轴标签输入框
            x2label_lbl.setVisible(self.settings_state.get('enable_twinx', False))
            x2label_edit.setVisible(self.settings_state.get('enable_twinx', False))
            y2label_lbl.setVisible(self.settings_state.get('enable_twiny', False))
            y2label_edit.setVisible(self.settings_state.get('enable_twiny', False))
            
            # 双轴开关切换时显示/隐藏对应标签输入框
            def update_twin_labels_visibility():
                enable_x = twin_x_cb.isChecked()
                enable_y = twin_y_cb.isChecked()
                x2label_lbl.setVisible(enable_x)
                x2label_edit.setVisible(enable_x)
                y2label_lbl.setVisible(enable_y)
                y2label_edit.setVisible(enable_y)
            
            twin_x_cb.toggled.connect(update_twin_labels_visibility)
            twin_y_cb.toggled.connect(update_twin_labels_visibility)

            # 切换字体时自动清空标签，避免跨字体混用的排版问题
            def on_font_changed(_text: str):
                xlabel_edit.clear()
                ylabel_edit.clear()
                x2label_edit.clear()
                y2label_edit.clear()
                self.settings_state['xlabel'] = ''
                self.settings_state['ylabel'] = ''
                self.settings_state['x2label'] = ''
                self.settings_state['y2label'] = ''
                # 同时清空图例自定义标签，避免跨字体混用
                self.settings_state['legend_labels'] = {}
                if hasattr(self, '_legend_labels'):
                    self._legend_labels = {}
                if legend_table is not None:
                    for i in range(legend_table.rowCount()):
                        w = legend_table.cellWidget(i, 1)
                        if w:
                            w.clear()

            font_combo.currentTextChanged.connect(on_font_changed)
            
            # 字号单独一行
            axis_row3 = QHBoxLayout()
            axis_row3.addWidget(QLabel("字号:"))
            fontsize_spin = QSpinBox()
            fontsize_spin.setRange(8, 48)
            fontsize_spin.setValue(self.settings_state['fontsize'])
            axis_row3.addWidget(fontsize_spin)
            axis_row3.addStretch()
            axis_layout.addLayout(axis_row3)

            # 标签与坐标轴的间距（pad）- 分开设置X和Y
            axis_row4 = QHBoxLayout()
            axis_row4.addWidget(QLabel("X轴标签间距:"))
            xlabel_pad_spin = QSpinBox()
            xlabel_pad_spin.setRange(0, 50)
            xlabel_pad_spin.setValue(self.settings_state.get('xlabel_pad', 3))
            xlabel_pad_spin.setMaximumWidth(80)
            axis_row4.addWidget(xlabel_pad_spin)
            axis_row4.addSpacing(12)
            axis_row4.addWidget(QLabel("Y轴标签间距:"))
            ylabel_pad_spin = QSpinBox()
            ylabel_pad_spin.setRange(0, 50)
            ylabel_pad_spin.setValue(self.settings_state.get('ylabel_pad', 0))
            ylabel_pad_spin.setMaximumWidth(80)
            axis_row4.addWidget(ylabel_pad_spin)
            axis_row4.addStretch()
            axis_layout.addLayout(axis_row4)
            
            group_axis.setLayout(axis_layout)
            s_layout.addWidget(group_axis)

            # 插入符号区域
            group_sym = QGroupBox("插入符号")
            sym_layout = QVBoxLayout()
            
            # 记录最后一次点击的编辑框
            last_edit = xlabel_edit
            
            # 为坐标轴标签编辑框添加焦点事件，记录当前编辑框
            def on_xlabel_focus():
                nonlocal last_edit
                last_edit = xlabel_edit
            
            def on_ylabel_focus():
                nonlocal last_edit
                last_edit = ylabel_edit
            
            def on_x2label_focus():
                nonlocal last_edit
                last_edit = x2label_edit
            
            def on_y2label_focus():
                nonlocal last_edit
                last_edit = y2label_edit
            
            xlabel_edit.mousePressEvent = lambda e: (on_xlabel_focus(), QLineEdit.mousePressEvent(xlabel_edit, e))
            ylabel_edit.mousePressEvent = lambda e: (on_ylabel_focus(), QLineEdit.mousePressEvent(ylabel_edit, e))
            x2label_edit.mousePressEvent = lambda e: (on_x2label_focus(), QLineEdit.mousePressEvent(x2label_edit, e))
            y2label_edit.mousePressEvent = lambda e: (on_y2label_focus(), QLineEdit.mousePressEvent(y2label_edit, e))
            
            # 工具函数：插入符号到当前焦点编辑框（智能包裹$）
            def _insert_symbol(tex: str):
                # 获取当前焦点的输入框（支持主窗口和对话框内的输入框）
                focused = QApplication.focusWidget()
                if isinstance(focused, QLineEdit):
                    edit = focused
                else:
                    # 回退到记录的最后一个编辑框
                    edit = last_edit
                
                cur = edit.cursorPosition()
                s = edit.text()
                
                # 检查是否是对话框内的输入框（通过自定义属性标记）
                is_sqrt_dialog = edit.property("is_sqrt_dialog") or False
                is_template_dialog = edit.property("is_template_dialog") or False

                # 在模板/根号对话框中，命令型符号默认加空格，避免与后续字母数字粘连
                if is_sqrt_dialog or is_template_dialog:
                    tex = _protect_cmd_for_template(tex)
                
                if is_sqrt_dialog or is_template_dialog:
                    # 在对话框内，直接插入 LaTeX 命令，不包裹 $
                    symbol_text = tex
                else:
                    # 检查光标前后是否已在数学模式内（查找最近的$符号）
                    before = s[:cur]
                    after = s[cur:]
                    dollar_count_before = before.count('$')
                    
                    # 如果前面有奇数个$，说明已在数学模式内，不需要包裹
                    if dollar_count_before % 2 == 1:
                        symbol_text = tex
                    else:
                        symbol_text = f"${tex}$"
                
                edit.setText(s[:cur] + symbol_text + s[cur:])
                # 光标移动到插入的符号后面
                edit.setCursorPosition(cur + len(symbol_text))

            # 工具函数：插入纯文本符号（不包裹数学模式）
            def _insert_plain_symbol(text: str):
                edit = last_edit
                cur = edit.cursorPosition()
                s = edit.text()
                edit.setText(s[:cur] + text + s[cur:])
                edit.setCursorPosition(cur + len(text))

            # 正体格式化：LaTeX 模式用 \up 前缀，mathtext 模式用 \mathregular
            def format_upright(content: str) -> str:
                use_latex = plt.rcParams.get('text.usetex', False)
                if use_latex:
                    c = content.strip()
                    if c.startswith('\\'):
                        c = c[1:]
                    lower_greek = {
                        "alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta", "iota", "kappa",
                        "lambda", "mu", "nu", "xi", "pi", "rho", "sigma", "tau", "upsilon", "phi", "chi",
                        "psi", "omega", "varepsilon", "vartheta", "varpi", "varrho", "varsigma", "varphi"
                    }
                    upper_greek = {
                        "Gamma", "Delta", "Theta", "Lambda", "Xi", "Pi", "Sigma", "Upsilon", "Phi", "Psi", "Omega"
                    }
                    if c in lower_greek:
                        return f"\\up{c}"
                    if c in upper_greek:
                        return f"\\{c}"
                    return f"\\text{{{content}}}"
                return f"\\mathregular{{{content}}}"

            # 模板对话框内的命令符号后追加空格，避免与后续字母数字连在一起
            def _protect_cmd_for_template(tex: str) -> str:
                if tex.endswith(" ") or tex.endswith("{}"):
                    return tex
                if tex.startswith("\\"):
                    body = tex[1:]
                    if body.isalpha():
                        return tex + " "
                return tex
            icon_dir = os.path.join(os.path.dirname(__file__), "symbol_icons")

            def _load_icon(category: str, name: str):
                path = os.path.join(icon_dir, category, f"{name}.svg")
                if os.path.exists(path):
                    return QIcon(path)
                return None

            # 完整希腊字母（小写）
            greek_lower_row = QHBoxLayout()
            greek_lower_row.addWidget(QLabel("希腊小:"))
            from qtawesome import icon as qta_icon
            # 使用数学符号字体显示，配合Tooltip说明
            greek_lower = [
                ("α", "\\alpha"), ("β", "\\beta"), ("γ", "\\gamma"), ("δ", "\\delta"),
                ("ε", "\\epsilon"), ("ζ", "\\zeta"), ("η", "\\eta"), ("θ", "\\theta"),
                ("ι", "\\iota"), ("κ", "\\kappa"), ("λ", "\\lambda"), ("μ", "\\mu"),
                ("ν", "\\nu"), ("ξ", "\\xi"), ("ο", "\\omicron"), ("π", "\\pi"),
                ("ρ", "\\rho"), ("σ", "\\sigma"), ("τ", "\\tau"), ("υ", "\\upsilon"),
                ("φ", "\\phi"), ("χ", "\\chi"), ("ψ", "\\psi"), ("ω", "\\omega")
            ]
            greek_lower_names = {
                "α": "alpha", "β": "beta", "γ": "gamma", "δ": "delta",
                "ε": "epsilon", "ζ": "zeta", "η": "eta", "θ": "theta",
                "ι": "iota", "κ": "kappa", "λ": "lambda", "μ": "mu",
                "ν": "nu", "ξ": "xi", "ο": "omicron", "π": "pi",
                "ρ": "rho", "σ": "sigma", "τ": "tau", "υ": "upsilon",
                "φ": "phi", "χ": "chi", "ψ": "psi", "ω": "omega"
            }
            for sym, tex in greek_lower:
                btn = QPushButton()
                btn.setFixedWidth(32)
                btn.setFixedHeight(32)
                btn.setToolTip(greek_lower_names[sym])
                icon_obj = _load_icon("greek_lower", greek_lower_names[sym])
                if icon_obj:
                    btn.setIcon(icon_obj)
                    btn.setIconSize(QSize(24, 24))
                else:
                    btn.setText(sym)
                    btn.setFont(__import__('PySide6.QtGui', fromlist=['QFont']).QFont(None, 14))
                def make_handler(t=tex):
                    return lambda: _insert_symbol(t)
                btn.clicked.connect(make_handler())
                greek_lower_row.addWidget(btn)
            greek_lower_row.addStretch()
            sym_layout.addLayout(greek_lower_row)

            # 完整希腊字母（大写）
            greek_upper_row = QHBoxLayout()
            greek_upper_row.addWidget(QLabel("希腊大:"))
            greek_upper = [
                ("Γ", "\\Gamma"), ("Δ", "\\Delta"), ("Θ", "\\Theta"), ("Λ", "\\Lambda"),
                ("Ξ", "\\Xi"), ("Π", "\\Pi"), ("Σ", "\\Sigma"), ("Υ", "\\Upsilon"),
                ("Φ", "\\Phi"), ("Ψ", "\\Psi"), ("Ω", "\\Omega")
            ]
            greek_upper_names = {
                "Γ": "Gamma", "Δ": "Delta", "Θ": "Theta", "Λ": "Lambda",
                "Ξ": "Xi", "Π": "Pi", "Σ": "Sigma", "Υ": "Upsilon",
                "Φ": "Phi", "Ψ": "Psi", "Ω": "Omega"
            }
            for sym, tex in greek_upper:
                btn = QPushButton()
                btn.setFixedWidth(32)
                btn.setFixedHeight(32)
                btn.setToolTip(greek_upper_names[sym])
                icon_obj = _load_icon("greek_upper", greek_upper_names[sym])
                if icon_obj:
                    btn.setIcon(icon_obj)
                    btn.setIconSize(QSize(24, 24))
                else:
                    btn.setText(sym)
                    btn.setFont(__import__('PySide6.QtGui', fromlist=['QFont']).QFont(None, 14))
                def make_handler(t=tex):
                    return lambda: _insert_symbol(t)
                btn.clicked.connect(make_handler())
                greek_upper_row.addWidget(btn)

            greek_upper_row.addStretch()
            sym_layout.addLayout(greek_upper_row)

            # 数学符号信息
            math_names = {
                "±": "pm", "∓": "mp", "×": "times", "÷": "div",
                "≈": "approx", "≠": "neq", "≤": "leq", "≥": "geq",
                "∞": "infty", "∂": "partial", "∫": "int", "√": "sqrt",
                "∑": "sum", "∏": "prod", "∘": "circ", "°": "degree",
                "⊥": "perp", "∥": "parallel", "∈": "in", "∉": "notin",
                "⊂": "subset", "⊃": "supset", "∩": "cap", "∪": "cup",
                "∀": "forall", "∃": "exists", "¬": "neg", "∧": "wedge",
                "∨": "vee", "⇒": "Rightarrow", "⇔": "Leftrightarrow", "∴": "therefore"
            }

            # 数学符号
            math_row = QHBoxLayout()
            math_row.addWidget(QLabel("数学 :"))
            math_buttons = [
                ("±", "\\pm"), ("∓", "\\mp"), ("×", "\\times"), ("÷", "\\div"),
                ("≈", "\\approx"), ("≠", "\\neq"), ("≤", "\\leq"), ("≥", "\\geq"),
                ("∞", "\\infty"), ("∂", "\\partial"), ("∫", "\\int"), ("√", "\\sqrt"),
                ("∑", "\\sum"), ("∏", "\\prod"), ("∘", "\\circ"), ("°", "^\\circ"),
                ("⊥", "\\perp"), ("∥", "\\parallel"), ("∈", "\\in"), ("∉", "\\notin"),
                ("⊂", "\\subset"), ("⊃", "\\supset"), ("∩", "\\cap"), ("∪", "\\cup"),
                ("∀", "\\forall"), ("∃", "\\exists"), ("¬", "\\neg"), ("∧", "\\wedge"),
                ("∨", "\\vee"), ("⇒", "\\Rightarrow"), ("⇔", "\\Leftrightarrow"), ("∴", "\\therefore")
            ]
            for sym, tex in math_buttons:
                btn = QPushButton()
                btn.setFixedWidth(32)
                btn.setFixedHeight(32)
                btn.setToolTip(math_names[sym])
                icon_obj = _load_icon("math_symbols", math_names[sym])
                if icon_obj:
                    btn.setIcon(icon_obj)
                    btn.setIconSize(QSize(24, 24))
                else:
                    btn.setText(sym)
                    btn.setFont(__import__('PySide6.QtGui', fromlist=['QFont']).QFont(None, 14))
                def make_handler(t=tex):
                    return lambda: _insert_symbol(t)
                btn.clicked.connect(make_handler())
                math_row.addWidget(btn)
            math_row.addStretch()
            sym_layout.addLayout(math_row)

            # 上下标和根号模板
            tpl_row = QHBoxLayout()
            tpl_row.addWidget(QLabel("其它 :"))
            tpl_buttons = [
                ("上标", "^{  }"),
                ("下标", "_{  }"),
                ("正体", "\\mathregular{ }"),
                ("根号", "\\sqrt{  }")
            ]
            for disp, tex in tpl_buttons:
                btn = QPushButton(disp)
                btn.setFixedWidth(50)
                btn.setFixedHeight(32)
                btn.setToolTip(disp)

                def make_handler(t=tex, label=disp):
                    def handler():
                        nonlocal last_edit
                        original_edit = last_edit
                        
                        if label == "上标":
                            # 上标对话框
                            from PySide6.QtWidgets import QInputDialog, QDialog, QVBoxLayout, QHBoxLayout, QLineEdit, QPushButton, QLabel
                            content_dialog = QDialog(original_edit)
                            content_dialog.setWindowTitle("设置上标")
                            content_dialog.setMinimumWidth(450)
                            content_dialog.setModal(False)
                            content_dialog.setWindowFlags(content_dialog.windowFlags() & ~Qt.WindowContextHelpButtonHint)
                            dlg_layout = QVBoxLayout()
                            
                            dlg_layout.addWidget(QLabel("底数（Base）："))
                            base_edit = QLineEdit()
                            base_edit.setProperty("is_template_dialog", True)
                            dlg_layout.addWidget(base_edit)
                            
                            dlg_layout.addWidget(QLabel("指数（Exponent）："))
                            exp_edit = QLineEdit()
                            exp_edit.setProperty("is_template_dialog", True)
                            dlg_layout.addWidget(exp_edit)
                            
                            # 添加焦点事件处理，使得焦点变化时自动更新 last_edit
                            def make_focus_handler(edit):
                                def focusInEvent(e):
                                    nonlocal last_edit
                                    last_edit = edit
                                    QLineEdit.focusInEvent(edit, e)
                                return focusInEvent
                            
                            base_edit.focusInEvent = make_focus_handler(base_edit)
                            exp_edit.focusInEvent = make_focus_handler(exp_edit)
                            
                            dlg_layout.addWidget(QLabel("💡 提示：可点击主窗口的符号按钮插入"))
                            
                            btn_row = QHBoxLayout()
                            btn_row.addStretch()
                            
                            def on_ok():
                                nonlocal last_edit
                                base = base_edit.text()
                                exp = exp_edit.text()
                                final_t = f"{base}^{{{exp}}}"
                                last_edit = original_edit
                                
                                edit = original_edit
                                cur = edit.cursorPosition()
                                s = edit.text()
                                before = s[:cur]
                                after = s[cur:]
                                dollar_count_before = before.count('$')
                                
                                # 检查原始编辑框是否在对话框中（有特殊属性标记）
                                is_in_dialog = (edit.property("is_template_dialog") or edit.property("is_sqrt_dialog"))
                                
                                if is_in_dialog or dollar_count_before % 2 == 1:
                                    symbol_text = final_t
                                else:
                                    symbol_text = f"${final_t}$"
                                
                                edit.setText(before + symbol_text + after)
                                edit.setCursorPosition(cur + len(symbol_text))
                                content_dialog.close()
                            
                            def on_cancel():
                                nonlocal last_edit
                                last_edit = original_edit
                                content_dialog.close()
                            
                            ok_btn = QPushButton("确定")
                            ok_btn.setStyleSheet("padding: 6px 20px; background-color: #4CAF50; color: white; border-radius: 4px;")
                            ok_btn.clicked.connect(on_ok)
                            cancel_btn = QPushButton("取消")
                            cancel_btn.setStyleSheet("padding: 6px 20px; background-color: #9E9E9E; color: white; border-radius: 4px;")
                            cancel_btn.clicked.connect(on_cancel)
                            btn_row.addWidget(ok_btn)
                            btn_row.addWidget(cancel_btn)
                            dlg_layout.addLayout(btn_row)
                            
                            content_dialog.setLayout(dlg_layout)
                            last_edit = base_edit
                            base_edit.setFocus()
                            content_dialog.show()
                            return
                        
                        elif label == "下标":
                            # 下标对话框
                            from PySide6.QtWidgets import QInputDialog, QDialog, QVBoxLayout, QHBoxLayout, QLineEdit, QPushButton, QLabel
                            content_dialog = QDialog(original_edit)
                            content_dialog.setWindowTitle("设置下标")
                            content_dialog.setMinimumWidth(450)
                            content_dialog.setModal(False)
                            content_dialog.setWindowFlags(content_dialog.windowFlags() & ~Qt.WindowContextHelpButtonHint)
                            dlg_layout = QVBoxLayout()
                            
                            dlg_layout.addWidget(QLabel("底数（Base）："))
                            base_edit = QLineEdit()
                            base_edit.setProperty("is_template_dialog", True)
                            dlg_layout.addWidget(base_edit)
                            
                            dlg_layout.addWidget(QLabel("下标（Index）："))
                            index_edit = QLineEdit()
                            index_edit.setProperty("is_template_dialog", True)
                            dlg_layout.addWidget(index_edit)
                            
                            # 添加焦点事件处理，使得焦点变化时自动更新 last_edit
                            def make_focus_handler(edit):
                                def focusInEvent(e):
                                    nonlocal last_edit
                                    last_edit = edit
                                    QLineEdit.focusInEvent(edit, e)
                                return focusInEvent
                            
                            base_edit.focusInEvent = make_focus_handler(base_edit)
                            index_edit.focusInEvent = make_focus_handler(index_edit)
                            
                            dlg_layout.addWidget(QLabel("💡 提示：可点击主窗口的符号按钮插入"))
                            
                            btn_row = QHBoxLayout()
                            btn_row.addStretch()
                            
                            def on_ok():
                                nonlocal last_edit
                                base = base_edit.text()
                                index = index_edit.text()
                                final_t = f"{base}_{{{index}}}"
                                last_edit = original_edit
                                
                                edit = original_edit
                                cur = edit.cursorPosition()
                                s = edit.text()
                                before = s[:cur]
                                after = s[cur:]
                                dollar_count_before = before.count('$')
                                
                                # 检查原始编辑框是否在对话框中（有特殊属性标记）
                                is_in_dialog = (edit.property("is_template_dialog") or edit.property("is_sqrt_dialog"))
                                
                                if is_in_dialog or dollar_count_before % 2 == 1:
                                    symbol_text = final_t
                                else:
                                    symbol_text = f"${final_t}$"
                                
                                edit.setText(before + symbol_text + after)
                                edit.setCursorPosition(cur + len(symbol_text))
                                content_dialog.close()
                            
                            def on_cancel():
                                nonlocal last_edit
                                last_edit = original_edit
                                content_dialog.close()
                            
                            ok_btn = QPushButton("确定")
                            ok_btn.setStyleSheet("padding: 6px 20px; background-color: #4CAF50; color: white; border-radius: 4px;")
                            ok_btn.clicked.connect(on_ok)
                            cancel_btn = QPushButton("取消")
                            cancel_btn.setStyleSheet("padding: 6px 20px; background-color: #9E9E9E; color: white; border-radius: 4px;")
                            cancel_btn.clicked.connect(on_cancel)
                            btn_row.addWidget(ok_btn)
                            btn_row.addWidget(cancel_btn)
                            dlg_layout.addLayout(btn_row)
                            
                            content_dialog.setLayout(dlg_layout)
                            last_edit = base_edit
                            base_edit.setFocus()
                            content_dialog.show()
                            return
                        
                        elif label == "正体":
                            # 正体对话框
                            from PySide6.QtWidgets import QInputDialog, QDialog, QVBoxLayout, QHBoxLayout, QLineEdit, QPushButton, QLabel
                            content_dialog = QDialog(original_edit)
                            content_dialog.setWindowTitle("设置正体")
                            content_dialog.setMinimumWidth(450)
                            content_dialog.setModal(False)
                            content_dialog.setWindowFlags(content_dialog.windowFlags() & ~Qt.WindowContextHelpButtonHint)
                            dlg_layout = QVBoxLayout()
                            
                            dlg_layout.addWidget(QLabel("请输入要正体化的字符："))
                            content_edit = QLineEdit()
                            content_edit.setProperty("is_template_dialog", True)
                            dlg_layout.addWidget(content_edit)
                            
                            # 添加焦点事件处理，使得焦点变化时自动更新 last_edit
                            def make_focus_handler(edit):
                                def focusInEvent(e):
                                    nonlocal last_edit
                                    last_edit = edit
                                    QLineEdit.focusInEvent(edit, e)
                                return focusInEvent
                            
                            content_edit.focusInEvent = make_focus_handler(content_edit)
                            
                            dlg_layout.addWidget(QLabel("💡 提示：可点击主窗口的符号按钮插入符号"))
                            
                            btn_row = QHBoxLayout()
                            btn_row.addStretch()
                            
                            def on_ok():
                                nonlocal last_edit
                                content = content_edit.text()
                                final_t = format_upright(content)
                                last_edit = original_edit
                                
                                edit = original_edit
                                cur = edit.cursorPosition()
                                s = edit.text()
                                before = s[:cur]
                                after = s[cur:]
                                dollar_count_before = before.count('$')
                                
                                # 检查原始编辑框是否在对话框中（有特殊属性标记）
                                is_in_dialog = (edit.property("is_template_dialog") or edit.property("is_sqrt_dialog"))
                                
                                if is_in_dialog or dollar_count_before % 2 == 1:
                                    symbol_text = final_t
                                else:
                                    symbol_text = f"${final_t}$"
                                
                                edit.setText(before + symbol_text + after)
                                edit.setCursorPosition(cur + len(symbol_text))
                                content_dialog.close()
                            
                            def on_cancel():
                                nonlocal last_edit
                                last_edit = original_edit
                                content_dialog.close()
                            
                            ok_btn = QPushButton("确定")
                            ok_btn.setStyleSheet("padding: 6px 20px; background-color: #4CAF50; color: white; border-radius: 4px;")
                            ok_btn.clicked.connect(on_ok)
                            cancel_btn = QPushButton("取消")
                            cancel_btn.setStyleSheet("padding: 6px 20px; background-color: #9E9E9E; color: white; border-radius: 4px;")
                            cancel_btn.clicked.connect(on_cancel)
                            btn_row.addWidget(ok_btn)
                            btn_row.addWidget(cancel_btn)
                            dlg_layout.addLayout(btn_row)
                            
                            content_dialog.setLayout(dlg_layout)
                            last_edit = content_edit
                            content_edit.setFocus()
                            content_dialog.show()
                            return
                        
                        # 根号处理（保留原有逻辑）
                        current_t = t
                        if label == "根号":
                            try:
                                from PySide6.QtWidgets import QInputDialog, QDialog, QVBoxLayout, QHBoxLayout, QLineEdit, QPushButton, QLabel
                                # 次数：默认2（平方根不显示指数）
                                n, ok = QInputDialog.getInt(
                                    last_edit, "设置根号次数", "请输入开方次数 (默认2为平方根):", 2, 2, 99, 1
                                )
                                if not ok:
                                    return
                                
                                # 保存当前的 last_edit，以便对话框关闭后恢复
                                original_edit = last_edit
                                
                                # 非模态对话框：输入根号内容，可以点击主窗口的符号按钮
                                content_dialog = QDialog(last_edit)
                                content_dialog.setWindowTitle("设置被开方内容")
                                content_dialog.setMinimumWidth(450)
                                content_dialog.setModal(False)  # 关键：设置为非模态
                                content_dialog.setWindowFlags(content_dialog.windowFlags() & ~Qt.WindowContextHelpButtonHint)
                                dlg_layout = QVBoxLayout()
                                
                                info_label = QLabel("请输入根号内的内容（可留空，也可点击主窗口的符号按钮插入）：")
                                info_label.setWordWrap(True)
                                info_label.setStyleSheet("color: #2196F3; font-weight: bold; margin-bottom: 5px;")
                                dlg_layout.addWidget(info_label)
                                
                                content_edit = QLineEdit()
                                content_edit.setPlaceholderText("可留空或输入内容，支持点击主窗口符号按钮")
                                content_edit.setStyleSheet("padding: 8px; font-size: 12pt;")
                                content_edit.setProperty("is_sqrt_dialog", True)  # 标记为根号对话框输入框
                                dlg_layout.addWidget(content_edit)
                                
                                # 关键：将 last_edit 临时设置为对话框的输入框
                                last_edit = content_edit
                                
                                # 提示
                                tip_label = QLabel("💡 提示：现在可以直接点击主窗口的符号按钮插入符号了")
                                tip_label.setStyleSheet("color: #666; font-size: 10pt; margin-top: 8px;")
                                dlg_layout.addWidget(tip_label)
                                
                                # 确定/取消按钮
                                btn_row = QHBoxLayout()
                                btn_row.addStretch()
                                
                                def on_ok():
                                    content = content_edit.text()
                                    if n == 2:
                                        final_t = f"\\sqrt{{{content}}}"
                                    else:
                                        final_t = f"\\sqrt[{n}]{{{content}}}"
                                    
                                    # 恢复原始的 last_edit
                                    nonlocal last_edit
                                    last_edit = original_edit
                                    
                                    # 插入到原始编辑框
                                    edit = original_edit
                                    cur = edit.cursorPosition()
                                    s = edit.text()
                                    before = s[:cur]
                                    after = s[cur:]
                                    dollar_count_before = before.count('$')
                                    
                                    # 检查原始编辑框是否在对话框中（有特殊属性标记）
                                    is_in_dialog = (edit.property("is_template_dialog") or edit.property("is_sqrt_dialog"))
                                    
                                    if is_in_dialog or dollar_count_before % 2 == 1:
                                        symbol_text = final_t
                                    else:
                                        symbol_text = f"${final_t}$"
                                    
                                    edit.setText(before + symbol_text + after)
                                    edit.setCursorPosition(cur + len(symbol_text))
                                    content_dialog.close()
                                
                                def on_cancel():
                                    # 恢复原始的 last_edit
                                    nonlocal last_edit
                                    last_edit = original_edit
                                    content_dialog.close()
                                
                                ok_btn = QPushButton("确定")
                                ok_btn.setStyleSheet("padding: 6px 20px; background-color: #4CAF50; color: white; border-radius: 4px;")
                                ok_btn.clicked.connect(on_ok)
                                cancel_btn = QPushButton("取消")
                                cancel_btn.setStyleSheet("padding: 6px 20px; background-color: #9E9E9E; color: white; border-radius: 4px;")
                                cancel_btn.clicked.connect(on_cancel)
                                btn_row.addWidget(ok_btn)
                                btn_row.addWidget(cancel_btn)
                                dlg_layout.addLayout(btn_row)
                                
                                content_dialog.setLayout(dlg_layout)
                                
                                # 自动聚焦到输入框
                                content_edit.setFocus()
                                
                                # 非模态显示
                                content_dialog.show()
                                return  # 直接返回，不阻塞
                                
                            except Exception:
                                current_t = "\\sqrt{  }"

                        # 插入模板并将光标放到首个花括号内，便于继续输入
                        edit = last_edit
                        cur = edit.cursorPosition()
                        s = edit.text()

                        before = s[:cur]
                        after = s[cur:]
                        dollar_count_before = before.count('$')

                        if dollar_count_before % 2 == 1:
                            symbol_text = current_t
                            dollar_prefix = 0
                        else:
                            symbol_text = f"${current_t}$"
                            dollar_prefix = 1

                        new_text = before + symbol_text + after
                        edit.setText(new_text)

                        # 优先将光标放到指数位（如 \sqrt[n]{ }) 的方括号内；否则放到首个花括号内
                        bracket_idx = current_t.find('[')
                        brace_idx = current_t.find('{')
                        if bracket_idx >= 0 and (brace_idx < 0 or bracket_idx < brace_idx):
                            target_pos = cur + dollar_prefix + bracket_idx + 1
                        elif brace_idx >= 0:
                            target_pos = cur + dollar_prefix + brace_idx + 1
                        else:
                            target_pos = cur + len(symbol_text)
                        edit.setCursorPosition(target_pos)
                    return handler

                btn.clicked.connect(make_handler())
                tpl_row.addWidget(btn)
            tpl_row.addSpacing(10)
            tpl_hint = QLabel("提示: 在{ }内插入字符")
            tpl_hint.setStyleSheet("color: #666; font-size: 15px;")
            tpl_row.addWidget(tpl_hint)
            tpl_row.addStretch()
            sym_layout.addLayout(tpl_row)

            group_sym.setLayout(sym_layout)
            s_layout.addWidget(group_sym)

            # 图例设置
            group_legend = QGroupBox("图例")
            legend_layout = QVBoxLayout()
            
            legend_row1 = QHBoxLayout()
            legend_row1.addWidget(QLabel("字号:"))
            legend_fontsize_spin = QSpinBox()
            legend_fontsize_spin.setRange(6, 48)
            legend_fontsize_spin.setValue(self.settings_state.get('legend_fontsize', 28))
            legend_row1.addWidget(legend_fontsize_spin)
            legend_row1.addStretch()
            legend_layout.addLayout(legend_row1)
            
            # 位置关键字选择
            legend_row2 = QHBoxLayout()
            legend_row2.addWidget(QLabel("位置关键字:"))
            legend_loc_combo = QComboBox()
            legend_loc_combo.addItems([
                'lower right','best','upper right','upper left','lower left','right',
                'center left','center right','lower center','upper center','center'
            ])
            # 从设置中获取当前值
            cur_loc = self.settings_state.get('legend_loc', 'lower right')
            if cur_loc in ['best','upper right','upper left','lower left','lower right','right',
                          'center left','center right','lower center','upper center','center']:
                legend_loc_combo.setCurrentText(cur_loc)
            legend_loc_combo.setMinimumWidth(120)
            legend_row2.addWidget(legend_loc_combo)
            legend_row2.addStretch()
            legend_layout.addLayout(legend_row2)
            
            # 位置坐标输入
            legend_row3 = QHBoxLayout()
            legend_row3.addWidget(QLabel("位置坐标 (x, y):"))
            legend_coord_edit = QLineEdit()
            legend_coord_edit.setPlaceholderText("例如: 0.5, 0.5 (留空使用关键字)")
            # 如果当前设置是坐标格式，则显示在输入框中
            if cur_loc and cur_loc not in ['best','upper right','upper left','lower left','lower right','right',
                                           'center left','center right','lower center','upper center','center']:
                legend_coord_edit.setText(cur_loc)
            legend_row3.addWidget(legend_coord_edit)
            legend_row3.addStretch()
            legend_layout.addLayout(legend_row3)
            
            # 图例标签编辑区
            legend_layout.addWidget(QLabel("自定义图例标签（留空使用文件名）:"))
            from PySide6.QtWidgets import QTableWidget, QTableWidgetItem
            legend_table = QTableWidget()
            legend_table.setColumnCount(2)
            legend_table.setHorizontalHeaderLabels(["文件名", "图例标签"])
            
            # 获取当前选中的文件列表
            selected_files_list = [f for f in self.loaded_files if os.path.basename(f[0]) in self.settings_state['selected_files']]
            legend_table.setRowCount(len(selected_files_list))
            
            for i, (file_path, df) in enumerate(selected_files_list):
                file_name = os.path.basename(file_path)
                legend_table.setItem(i, 0, QTableWidgetItem(file_name))
                
                # 获取当前标签（可能是之前自定义的）
                current_label = self.settings_state['legend_labels'].get(file_name, '')
                label_edit = QLineEdit(current_label)
                label_edit.setPlaceholderText(os.path.splitext(file_name)[0])  # 默认显示文件名（无扩展名）
                legend_label_edits.append((file_name, label_edit))
                legend_table.setCellWidget(i, 1, label_edit)
            
            legend_table.resizeColumnsToContents()
            legend_table.setMinimumHeight(250)
            # 根据行数动态设置高度，大约每行40像素
            row_count = legend_table.rowCount()
            header_height = legend_table.horizontalHeader().height()
            estimated_height = header_height + (row_count * 40)
            legend_table.setMaximumHeight(max(estimated_height, 400))
            legend_layout.addWidget(legend_table)
            
            # 为图例标签编辑框添加焦点事件（使符号插入功能可用）
            for file_name, label_edit in legend_label_edits:
                def make_focus_handler(edit):
                    def on_focus():
                        nonlocal last_edit
                        last_edit = edit
                    return on_focus
                focus_handler = make_focus_handler(label_edit)
                label_edit.mousePressEvent = lambda e, h=focus_handler, le=label_edit: (h(), QLineEdit.mousePressEvent(le, e))
            
            group_legend.setLayout(legend_layout)
            s_layout.addWidget(group_legend)
            
            # 现在 target_combo 和 legend_table 都已创建，连接 itemSelectionChanged 信号以实现动态更新
            files_list.itemSelectionChanged.connect(update_select_all_checkbox)

            
            # 刻度设置
            group_tick = QGroupBox("刻度")
            tick_layout = QVBoxLayout()
            
            tick_row1 = QHBoxLayout()
            # 方向
            tick_row1.addWidget(QLabel("方向:"))
            tick_dir_combo = QComboBox()
            tick_dir_combo.addItems(['in', 'out', 'inout'])
            tick_dir_combo.setCurrentText(self.settings_state['tick_dir'])
            tick_row1.addWidget(tick_dir_combo)
            tick_row1.addStretch()
            tick_layout.addLayout(tick_row1)
            
            tick_row2 = QHBoxLayout()
            # 长度
            tick_row2.addWidget(QLabel("长度:"))
            tick_len_spin = QDoubleSpinBox()
            tick_len_spin.setRange(0.0, 20.0)
            tick_len_spin.setSingleStep(0.5)
            tick_len_spin.setValue(self.settings_state['tick_len'])
            tick_row2.addWidget(tick_len_spin)
            # 粗细
            tick_row2.addSpacing(12)
            tick_row2.addWidget(QLabel("粗细:"))
            tick_wid_spin = QDoubleSpinBox()
            tick_wid_spin.setRange(0.5, 5.0)
            tick_wid_spin.setSingleStep(0.5)
            tick_wid_spin.setValue(self.settings_state['tick_wid'])
            tick_row2.addWidget(tick_wid_spin)
            tick_row2.addStretch()
            tick_layout.addLayout(tick_row2)
            
            tick_row3 = QHBoxLayout()
            # 标签字号
            tick_row3.addWidget(QLabel("标签字号:"))
            tick_label_size_spin = QSpinBox()
            tick_label_size_spin.setRange(6, 48)
            tick_label_size_spin.setValue(self.settings_state['tick_label_size'])
            tick_row3.addWidget(tick_label_size_spin)
            # 弧度模式
            tick_row3.addSpacing(12)
            rad_check = QCheckBox("弧度(π刻度)")
            rad_check.setChecked(self.settings_state['radian_mode'])
            tick_row3.addWidget(rad_check)
            tick_row3.addStretch()
            tick_layout.addLayout(tick_row3)
            
            # Major刻度设置（放在副刻度之前）
            tick_row4 = QHBoxLayout()
            tick_row4.addWidget(QLabel("X轴主间隔:"))
            major_x_interval_edit = QLineEdit(self.settings_state.get('major_x_interval', ''))
            major_x_interval_edit.setPlaceholderText("例如: 1.0")
            major_x_interval_edit.setMaximumWidth(80)
            tick_row4.addWidget(major_x_interval_edit)
            tick_row4.addSpacing(12)
            tick_row4.addWidget(QLabel("Y轴主间隔:"))
            major_y_interval_edit = QLineEdit(self.settings_state.get('major_y_interval', ''))
            major_y_interval_edit.setPlaceholderText("例如: 1.0")
            major_y_interval_edit.setMaximumWidth(80)
            tick_row4.addWidget(major_y_interval_edit)
            tick_row4.addStretch()
            tick_layout.addLayout(tick_row4)

            # 副刻度设置
            tick_row4b = QHBoxLayout()
            minor_check = QCheckBox("启用副刻度")
            minor_check.setChecked(bool(self.settings_state.get('minor_ticks', False)))
            tick_row4b.addWidget(minor_check)
            tick_row4b.addSpacing(12)
            tick_row4b.addWidget(QLabel("X轴副间隔:"))
            minor_x_interval_edit = QLineEdit(self.settings_state.get('minor_x_interval', ''))
            minor_x_interval_edit.setPlaceholderText("例如: 0.5")
            minor_x_interval_edit.setMaximumWidth(80)
            tick_row4b.addWidget(minor_x_interval_edit)
            tick_row4b.addSpacing(12)
            tick_row4b.addWidget(QLabel("Y轴副间隔:"))
            minor_y_interval_edit = QLineEdit(self.settings_state.get('minor_y_interval', ''))
            minor_y_interval_edit.setPlaceholderText("例如: 0.5")
            minor_y_interval_edit.setMaximumWidth(80)
            tick_row4b.addWidget(minor_y_interval_edit)
            tick_row4b.addStretch()

            def _update_minor_enabled(state=None):
                enabled = minor_check.isChecked()
                minor_x_interval_edit.setEnabled(enabled)
                minor_y_interval_edit.setEnabled(enabled)
            minor_check.toggled.connect(_update_minor_enabled)
            _update_minor_enabled()
            tick_layout.addLayout(tick_row4b)

            # 轴范围设置 xlim/ylim
            tick_row_limits = QHBoxLayout()
            tick_row_limits.addWidget(QLabel("X范围:"))
            x_min_edit = QLineEdit(self.settings_state.get('x_min', ''))
            x_min_edit.setPlaceholderText("最小值")
            x_min_edit.setMaximumWidth(90)
            tick_row_limits.addWidget(x_min_edit)
            tick_row_limits.addWidget(QLabel("~"))
            x_max_edit = QLineEdit(self.settings_state.get('x_max', ''))
            x_max_edit.setPlaceholderText("最大值")
            x_max_edit.setMaximumWidth(90)
            tick_row_limits.addWidget(x_max_edit)
            tick_row_limits.addSpacing(16)
            tick_row_limits.addWidget(QLabel("Y范围:"))
            y_min_edit = QLineEdit(self.settings_state.get('y_min', ''))
            y_min_edit.setPlaceholderText("最小值")
            y_min_edit.setMaximumWidth(90)
            tick_row_limits.addWidget(y_min_edit)
            tick_row_limits.addWidget(QLabel("~"))
            y_max_edit = QLineEdit(self.settings_state.get('y_max', ''))
            y_max_edit.setPlaceholderText("最大值")
            y_max_edit.setMaximumWidth(90)
            tick_row_limits.addWidget(y_max_edit)
            tick_row_limits.addStretch()
            tick_layout.addLayout(tick_row_limits)
            
            # 双轴范围设置
            tick_row_limits2 = QHBoxLayout()
            x2_range_lbl = QLabel("顶部X范围:")
            tick_row_limits2.addWidget(x2_range_lbl)
            x2_min_edit = QLineEdit(self.settings_state.get('x2_min', ''))
            x2_min_edit.setPlaceholderText("最小值")
            x2_min_edit.setMaximumWidth(90)
            tick_row_limits2.addWidget(x2_min_edit)
            x2_tilde1 = QLabel("~")
            tick_row_limits2.addWidget(x2_tilde1)
            x2_max_edit = QLineEdit(self.settings_state.get('x2_max', ''))
            x2_max_edit.setPlaceholderText("最大值")
            x2_max_edit.setMaximumWidth(90)
            tick_row_limits2.addWidget(x2_max_edit)
            tick_row_limits2.addSpacing(16)
            y2_range_lbl = QLabel("右侧Y范围:")
            tick_row_limits2.addWidget(y2_range_lbl)
            y2_min_edit = QLineEdit(self.settings_state.get('y2_min', ''))
            y2_min_edit.setPlaceholderText("最小值")
            y2_min_edit.setMaximumWidth(90)
            tick_row_limits2.addWidget(y2_min_edit)
            y2_tilde1 = QLabel("~")
            tick_row_limits2.addWidget(y2_tilde1)
            y2_max_edit = QLineEdit(self.settings_state.get('y2_max', ''))
            y2_max_edit.setPlaceholderText("最大值")
            y2_max_edit.setMaximumWidth(90)
            tick_row_limits2.addWidget(y2_max_edit)
            tick_row_limits2.addStretch()
            tick_layout.addLayout(tick_row_limits2)
            
            # 默认隐藏双轴范围输入
            x2_range_lbl.setVisible(self.settings_state.get('enable_twinx', False))
            x2_min_edit.setVisible(self.settings_state.get('enable_twinx', False))
            x2_tilde1.setVisible(self.settings_state.get('enable_twinx', False))
            x2_max_edit.setVisible(self.settings_state.get('enable_twinx', False))
            y2_range_lbl.setVisible(self.settings_state.get('enable_twiny', False))
            y2_min_edit.setVisible(self.settings_state.get('enable_twiny', False))
            y2_tilde1.setVisible(self.settings_state.get('enable_twiny', False))
            y2_max_edit.setVisible(self.settings_state.get('enable_twiny', False))
            
            # 同步双轴范围可见性
            def update_twin_ranges_visibility():
                enable_x = twin_x_cb.isChecked()
                enable_y = twin_y_cb.isChecked()
                x2_range_lbl.setVisible(enable_x)
                x2_min_edit.setVisible(enable_x)
                x2_tilde1.setVisible(enable_x)
                x2_max_edit.setVisible(enable_x)
                y2_range_lbl.setVisible(enable_y)
                y2_min_edit.setVisible(enable_y)
                y2_tilde1.setVisible(enable_y)
                y2_max_edit.setVisible(enable_y)
            
            twin_x_cb.toggled.connect(update_twin_ranges_visibility)
            twin_y_cb.toggled.connect(update_twin_ranges_visibility)

            
            group_tick.setLayout(tick_layout)
            s_layout.addWidget(group_tick)

            # 按钮
            row_btn = QHBoxLayout()
            btn_ok = QPushButton("确定")
            btn_cancel = QPushButton("取消")
            
            def apply_settings():
                # 保存设置到状态字典
                self.settings_state['selected_files'] = {it.text() for it in files_list.selectedItems()}
                # 线型
                ls_text = ls_combo.currentText()
                self.settings_state['linestyle'] = 'None' if ls_text == '无' else ls_text
                # 颜色模式和值
                self.settings_state['color_scheme_type'] = color_mode_combo.currentText()
                if self.settings_state['color_scheme_type'] == '固定颜色':
                    self.settings_state['color'] = color_combo.currentData()
                else:
                    self.settings_state['colormap'] = colormap_combo.currentText()
                # Marker与大小
                marker_text = marker_combo.currentText()
                self.settings_state['marker'] = '' if marker_text == '无' else marker_text
                self.settings_state['markersize'] = ms_spin.value()
                # 线宽
                self.settings_state['linewidth'] = lw_spin.value()
                # 边框宽度
                self.settings_state['frame_width'] = frame_spin.value()
                # 坐标标签与字体
                self.settings_state['xlabel'] = xlabel_edit.text().strip()
                self.settings_state['ylabel'] = ylabel_edit.text().strip()
                self.settings_state['x2label'] = x2label_edit.text().strip()
                self.settings_state['y2label'] = y2label_edit.text().strip()
                self.settings_state['fontsize'] = fontsize_spin.value()
                # 图尺寸
                self.settings_state['fig_w'] = fig_w_spin.value()
                self.settings_state['fig_h'] = fig_h_spin.value()
                self.settings_state['xlabel_pad'] = xlabel_pad_spin.value()
                self.settings_state['ylabel_pad'] = ylabel_pad_spin.value()
                self.settings_state['fontfamily'] = font_combo.currentText()
                self.settings_state['enable_twinx'] = twin_x_cb.isChecked()
                self.settings_state['enable_twiny'] = twin_y_cb.isChecked()

                # 应用字体与 LaTeX 设置
                if self.settings_state['fontfamily'] == 'Helvetica':
                    self.settings_state['use_latex'] = True
                    plt.rcParams['text.usetex'] = True
                    plt.rcParams['font.family'] = 'sans-serif'
                    plt.rcParams['font.sans-serif'] = ['Helvetica']
                    plt.rcParams['text.latex.preamble'] = r'''
                    \let\jmath\undefined
                    \usepackage{helvet}
                    \usepackage{sfmath}
                    \usepackage{amsmath}
                    \usepackage{upgreek}
                    \usepackage{amssymb}
                    '''
                else:
                    self.settings_state['use_latex'] = False
                    plt.rcParams['text.usetex'] = False
                    plt.rcParams['font.family'] = 'serif'
                    plt.rcParams['font.serif'] = ['Times New Roman', 'Times', 'DejaVu Serif']
    
                # 刻度设置
                self.settings_state['tick_dir'] = tick_dir_combo.currentText()
                self.settings_state['tick_len'] = tick_len_spin.value()
                self.settings_state['tick_wid'] = tick_wid_spin.value()
                self.settings_state['tick_label_size'] = tick_label_size_spin.value()
                self.settings_state['radian_mode'] = rad_check.isChecked()
                # 副刻度设置
                self.settings_state['minor_ticks'] = minor_check.isChecked()
                self.settings_state['minor_x_interval'] = minor_x_interval_edit.text().strip()
                self.settings_state['minor_y_interval'] = minor_y_interval_edit.text().strip()
                # 单曲线样式由“保存到该曲线”按钮即时写入 self.settings_state['per_series_style']
                # Major刻度设置
                self.settings_state['major_x_interval'] = major_x_interval_edit.text().strip()
                self.settings_state['major_y_interval'] = major_y_interval_edit.text().strip()
                # xlim/ylim 设置
                self.settings_state['x_min'] = x_min_edit.text().strip()
                self.settings_state['x_max'] = x_max_edit.text().strip()
                self.settings_state['y_min'] = y_min_edit.text().strip()
                self.settings_state['y_max'] = y_max_edit.text().strip()
                self.settings_state['x2_min'] = x2_min_edit.text().strip()
                self.settings_state['x2_max'] = x2_max_edit.text().strip()
                self.settings_state['y2_min'] = y2_min_edit.text().strip()
                self.settings_state['y2_max'] = y2_max_edit.text().strip()
                # 图例设置
                self.settings_state['legend_fontsize'] = legend_fontsize_spin.value()
                # 图例字体改为使用全局字体
                self.settings_state['legend_fontfamily'] = self.settings_state['fontfamily']
                # 优先使用坐标输入，没有坐标时使用关键字
                coord_text = legend_coord_edit.text().strip()
                if coord_text:
                    self.settings_state['legend_loc'] = coord_text
                else:
                    self.settings_state['legend_loc'] = legend_loc_combo.currentText()
                # 保存自定义图例标签
                for file_name, label_edit in legend_label_edits:
                    custom_label = label_edit.text().strip()
                    if custom_label:
                        self.settings_state['legend_labels'][file_name] = custom_label
                    elif file_name in self.settings_state['legend_labels']:
                        # 如果清空了，则删除自定义标签
                        del self.settings_state['legend_labels'][file_name]
                settings_dlg.accept()
            
            btn_ok.clicked.connect(apply_settings)
            btn_cancel.clicked.connect(settings_dlg.reject)
            row_btn.addStretch()
            row_btn.addWidget(btn_ok)
            row_btn.addWidget(btn_cancel)
            main_layout.addLayout(row_btn)

            if settings_dlg.exec() == QDialog.Accepted:
                refresh_preview()

        btn_basic.clicked.connect(open_basic_settings)

        def gather_selected_data():
            curves = []
            enable_twinx = self.settings_state.get('enable_twinx', False)
            enable_twiny = self.settings_state.get('enable_twiny', False)
            x2col = self.settings_state.get('x2col', '')
            y2col = self.settings_state.get('y2col', '')
            
            for file_path, df in self.loaded_files:
                base = os.path.basename(file_path)
                if base not in self.settings_state['selected_files']:
                    continue
                if xcol in df.columns and ycol in df.columns:
                    x = pd.to_numeric(df[xcol], errors='coerce')
                    y = pd.to_numeric(df[ycol], errors='coerce')
                    # 发布预览的弧度模式转换
                    if self.settings_state['radian_mode']:
                        x = x * np.pi / 180.0
                    
                    # 收集双轴数据（如启用）
                    x2 = None
                    y2 = None
                    if enable_twinx and x2col and x2col in df.columns:
                        x2 = pd.to_numeric(df[x2col], errors='coerce')
                        if self.settings_state['radian_mode']:
                            x2 = x2 * np.pi / 180.0
                    if enable_twiny and y2col and y2col in df.columns:
                        y2 = pd.to_numeric(df[y2col], errors='coerce')
                    
                    curves.append((base, x, y, x2, y2))
            return curves

        def refresh_preview():
            # 删除旧的 canvas 和 figure，重新创建
            nonlocal preview_fig, preview_ax, preview_canvas
            try:
                if preview_canvas is not None:
                    preview_container_layout.removeWidget(preview_canvas)
                    preview_canvas.setParent(None)
                    preview_canvas.deleteLater()
                    preview_canvas = None
            except Exception:
                pass
            
            try:
                if preview_fig is not None:
                    preview_fig.clear()
                    # 关闭 figure（释放资源）
                    import matplotlib.pyplot as plt
                    plt.close(preview_fig)
                    preview_fig = None
                    preview_ax = None
            except Exception:
                pass
            
            # 创建新的 Figure（带最新的 figsize）
            from matplotlib.figure import Figure
            fig_w = float(self.settings_state.get('fig_w', 8.0))
            fig_h = float(self.settings_state.get('fig_h', 8.0))
            print(f"[DEBUG] 创建 Figure: fig_w={fig_w}, fig_h={fig_h}")  # 调试输出
            preview_fig = Figure(figsize=(fig_w, fig_h), dpi=100, facecolor='white')
            preview_ax = preview_fig.add_subplot(111, facecolor='white')
            preview_ax.grid(False)
            preview_canvas = FigureCanvas(preview_fig)  # 用普通 Canvas，不强制正方形
            
            # 强制设置 Canvas 的尺寸（根据 figsize）
            canvas_width = int(fig_w * 100)  # dpi=100
            canvas_height = int(fig_h * 100)
            preview_canvas.setFixedSize(canvas_width, canvas_height)
            
            preview_container_layout.addWidget(preview_canvas)
            preview_container.updateGeometry()
            preview_container.update()
            preview_container.repaint()
            
            # 局部应用字体与 LaTeX 设置（不影响主界面）
            from matplotlib import rc_context
            # 解析数值的工具，兼容全角与中文标点
            def _parse_float(txt: str):
                if not isinstance(txt, str):
                    return None
                s = unicodedata.normalize('NFKC', txt).strip()
                # 去除千分位逗号，将中文逗号替换为常见小数点误输入
                s = s.replace('，', '.').replace(',', '')
                try:
                    return float(s)
                except Exception:
                    return None
            
            
            # 不使用rc_context，直接使用全局LaTeX设置
            curves = gather_selected_data()
            
            # 检查是否启用双轴
            enable_twinx = self.settings_state.get('enable_twinx', False)
            enable_twiny = self.settings_state.get('enable_twiny', False)
            
            # 管理预览中的双轴：移除旧的 preview twin 轴（若存在），并根据设置创建/复用当前预览的 twin 轴
            # 移除上一次预览残留的 twin 轴（可能来自之前的 preview_fig）
            if getattr(self, '_preview_ax2_x', None) is not None:
                try:
                    self._preview_ax2_x.remove()
                except Exception:
                    pass
                self._preview_ax2_x = None
            if getattr(self, '_preview_ax2_y', None) is not None:
                try:
                    self._preview_ax2_y.remove()
                except Exception:
                    pass
                self._preview_ax2_y = None

            ax2_x = None  # 顶部X轴
            ax2_y = None  # 右侧Y轴
            # 根据当前 preview_ax 创建 twin 轴并保存到实例属性以便下一次管理
            if enable_twinx:
                try:
                    self._preview_ax2_x = preview_ax.twiny()
                    ax2_x = self._preview_ax2_x
                except Exception:
                    self._preview_ax2_x = None
                    ax2_x = None
            else:
                self._preview_ax2_x = None
            if enable_twiny:
                try:
                    self._preview_ax2_y = preview_ax.twinx()
                    ax2_y = self._preview_ax2_y
                except Exception:
                    self._preview_ax2_y = None
                    ax2_y = None
            else:
                self._preview_ax2_y = None
            
            # 每条曲线独立样式，若未设置则回落到全局；颜色默认使用 Matplotlib 循环或渐变色
            per_style = self.settings_state.get('per_series_style', {})
            
            # 根据颜色方案类型生成颜色列表
            colors = None
            if self.settings_state.get('color_scheme_type') == '渐变色' and len(curves) > 1:
                # 使用 colormap 生成渐变色
                import matplotlib.cm as cm
                colormap_name = self.settings_state.get('colormap', 'viridis')
                cmap = cm.get_cmap(colormap_name)
                colors = [cmap(i / (len(curves) - 1)) for i in range(len(curves))]

            for idx, curve_data in enumerate(curves):
                base, x, y = curve_data[0], curve_data[1], curve_data[2]
                x2 = curve_data[3] if len(curve_data) > 3 else None
                y2 = curve_data[4] if len(curve_data) > 4 else None
                
                # 使用自定义标签（如果有）或默认文件名
                display_label = self.settings_state['legend_labels'].get(base, os.path.splitext(base)[0])
                s = per_style.get(base, {})
                ls_val = s.get('linestyle', self.settings_state['linestyle'])
                if ls_val == 'None':
                    ls_val = None
                mk_val = s.get('marker', self.settings_state['marker'] or None)
                if mk_val == '':
                    mk_val = None
                ms_val = float(s.get('markersize', self.settings_state['markersize']))
                lw_val = float(s.get('linewidth', self.settings_state['linewidth']))
                
                # 颜色：优先使用单曲线自定义；否则根据方案选择
                if 'color' in s and s.get('color'):
                    color_val = s.get('color')
                elif len(curves) == 1:
                    color_val = self.settings_state['color']
                elif colors is not None:
                    # 使用渐变色方案
                    color_val = colors[idx]
                else:
                    # 默认 Matplotlib 颜色循环
                    color_val = None
                
                # 主轴绘图
                preview_ax.plot(
                    x, y,
                    linestyle=ls_val,
                    linewidth=lw_val,
                    marker=mk_val,
                    markersize=ms_val,
                    color=color_val,
                    label=display_label
                )
                
                # 双轴绘图（使用与主轴相同的样式参数）
                if enable_twinx and ax2_x is not None and x2 is not None:
                    ax2_x.plot(x2, y, linestyle=ls_val, linewidth=lw_val, marker=mk_val, markersize=ms_val, color=color_val)
                if enable_twiny and ax2_y is not None and y2 is not None:
                    ax2_y.plot(x, y2, linestyle=ls_val, linewidth=lw_val, marker=mk_val, markersize=ms_val, color=color_val)

            # 设置轴标签（使用 FontProperties：西文优先，中文回退）
            from matplotlib.font_manager import FontProperties
            font_family = self.settings_state.get('fontfamily', 'Helvetica')
            label_fp = FontProperties(family=_font_families(font_family), size=self.settings_state['fontsize'])
            if self.settings_state['xlabel']:
                preview_ax.set_xlabel(self.settings_state['xlabel'], fontproperties=label_fp, labelpad=self.settings_state.get('xlabel_pad', 3))
            if self.settings_state['ylabel']:
                preview_ax.set_ylabel(self.settings_state['ylabel'], fontproperties=label_fp, labelpad=self.settings_state.get('ylabel_pad', 0))

            # 设置双轴标签（同上，适当增加 labelpad）
            if ax2_x is not None and self.settings_state.get('x2label', ''):
                ax2_x.set_xlabel(self.settings_state['x2label'], fontproperties=label_fp, labelpad=self.settings_state.get('xlabel_pad', 3) + 10)
            if ax2_y is not None and self.settings_state.get('y2label', ''):
                ax2_y.set_ylabel(self.settings_state['y2label'], fontproperties=label_fp, labelpad=self.settings_state.get('ylabel_pad', 0) + 10)

            # 显示图例
            if len(curves) > 0:
                # 解析位置：支持关键字或坐标
                loc_text = self.settings_state.get('legend_loc', 'lower right').strip()
                keyword_locs = {
                    'best','upper right','upper left','lower left','lower right','right',
                    'center left','center right','lower center','upper center','center'
                }
                if loc_text in keyword_locs:
                    loc = loc_text
                else:
                    try:
                        coords = [float(x.strip()) for x in loc_text.split(',')]
                        if len(coords) == 2:
                            loc = tuple(coords)
                        else:
                            loc = 'best'
                    except Exception:
                        loc = 'best'
                
                # 图例字体设置：改为使用全局字体
                from matplotlib.font_manager import FontProperties
                legend_font = self.settings_state.get('fontfamily', 'Helvetica')
                # 为了支持中文，传入一个包含常见中文字体的备选列表
                font_prop = FontProperties(family=_font_families(legend_font), size=self.settings_state['legend_fontsize'])
                leg = preview_ax.legend(loc=loc, frameon=False, prop=font_prop)
                

            # 设置刻度字体
            for lbl in list(preview_ax.get_xticklabels()) + list(preview_ax.get_yticklabels()):
                lbl.set_fontfamily(font_family)
            
            # 设置双轴刻度字体与主轴一致
            if ax2_x is not None:
                for lbl in ax2_x.get_xticklabels():
                    lbl.set_fontfamily(font_family)
            if ax2_y is not None:
                for lbl in ax2_y.get_yticklabels():
                    lbl.set_fontfamily(font_family)

            # 设置边框宽度
            for spine in preview_ax.spines.values():
                spine.set_linewidth(self.settings_state['frame_width'])
            if ax2_x is not None:
                for spine in ax2_x.spines.values():
                    spine.set_linewidth(self.settings_state['frame_width'])
            if ax2_y is not None:
                for spine in ax2_y.spines.values():
                    spine.set_linewidth(self.settings_state['frame_width'])

            # 应用用户指定的坐标范围（xlim/ylim）
            try:
                x_min_val = _parse_float(self.settings_state.get('x_min', ''))
                x_max_val = _parse_float(self.settings_state.get('x_max', ''))
                if x_min_val is not None and x_max_val is not None and x_max_val > x_min_val:
                    preview_ax.set_xlim(x_min_val, x_max_val)
                elif x_min_val is not None and x_max_val is None:
                    cur_xlim = preview_ax.get_xlim()
                    preview_ax.set_xlim(x_min_val, cur_xlim[1])
                elif x_max_val is not None and x_min_val is None:
                    cur_xlim = preview_ax.get_xlim()
                    preview_ax.set_xlim(cur_xlim[0], x_max_val)
            except Exception:
                pass

            try:
                y_min_val = _parse_float(self.settings_state.get('y_min', ''))
                y_max_val = _parse_float(self.settings_state.get('y_max', ''))
                if y_min_val is not None and y_max_val is not None and y_max_val > y_min_val:
                    preview_ax.set_ylim(y_min_val, y_max_val)
                elif y_min_val is not None and y_max_val is None:
                    cur_ylim = preview_ax.get_ylim()
                    preview_ax.set_ylim(y_min_val, cur_ylim[1])
                elif y_max_val is not None and y_min_val is None:
                    cur_ylim = preview_ax.get_ylim()
                    preview_ax.set_ylim(cur_ylim[0], y_max_val)
            except Exception:
                pass
            
            # 应用双轴范围
            if ax2_x is not None:
                try:
                    x2_min_val = _parse_float(self.settings_state.get('x2_min', ''))
                    x2_max_val = _parse_float(self.settings_state.get('x2_max', ''))
                    if x2_min_val is not None and x2_max_val is not None and x2_max_val > x2_min_val:
                        ax2_x.set_xlim(x2_min_val, x2_max_val)
                    elif x2_min_val is not None and x2_max_val is None:
                        cur_xlim = ax2_x.get_xlim()
                        ax2_x.set_xlim(x2_min_val, cur_xlim[1])
                    elif x2_max_val is not None and x2_min_val is None:
                        cur_xlim = ax2_x.get_xlim()
                        ax2_x.set_xlim(cur_xlim[0], x2_max_val)
                except Exception:
                    pass
            
            if ax2_y is not None:
                try:
                    y2_min_val = _parse_float(self.settings_state.get('y2_min', ''))
                    y2_max_val = _parse_float(self.settings_state.get('y2_max', ''))
                    if y2_min_val is not None and y2_max_val is not None and y2_max_val > y2_min_val:
                        ax2_y.set_ylim(y2_min_val, y2_max_val)
                    elif y2_min_val is not None and y2_max_val is None:
                        cur_ylim = ax2_y.get_ylim()
                        ax2_y.set_ylim(y2_min_val, cur_ylim[1])
                    elif y2_max_val is not None and y2_min_val is None:
                        cur_ylim = ax2_y.get_ylim()
                        ax2_y.set_ylim(cur_ylim[0], y2_max_val)
                except Exception:
                    pass

            # 推迟绘制，避免阻塞；动态pad基于当前标签文本
            
            # 动态计算pad：根据刻度标签最大长度
            max_label_len = 0
            for label in preview_ax.get_xticklabels() + preview_ax.get_yticklabels():
                text = label.get_text()
                max_label_len = max(max_label_len, len(text))
            
            # 根据标签长度动态设置pad：长标签需要更大间距
            dynamic_pad = 8 if max_label_len > 5 else 5

            # 应用刻度参数（主轴和双轴）
            tick_params_major = {
                'direction': self.settings_state['tick_dir'],
                'length': self.settings_state['tick_len'],
                'width': self.settings_state['tick_wid'],
                'labelsize': self.settings_state['tick_label_size'],
                'pad': dynamic_pad
            }
            tick_params_minor = {
                'which': 'minor',
                'direction': self.settings_state['tick_dir'],
                'length': self.settings_state['tick_len'] * 0.75,
                'width': self.settings_state['tick_wid']
            }
            
            # 应用刻度参数
            # 主轴：根据是否有双轴设置刻度位置
            if ax2_x is None and ax2_y is None:
                # 无双轴：四周显示刻度
                preview_ax.tick_params(axis='both', top=True, bottom=True, left=True, right=True, **tick_params_major)
                preview_ax.tick_params(axis='both', top=True, bottom=True, left=True, right=True, **tick_params_minor)
            elif ax2_x is not None and ax2_y is None:
                # 双 X 轴：X只在底部，Y在左右
                preview_ax.tick_params(axis='x', top=False, bottom=True, **tick_params_major)
                preview_ax.tick_params(axis='x', top=False, bottom=True, **tick_params_minor)
                preview_ax.tick_params(axis='y', left=True, right=True, **tick_params_major)
                preview_ax.tick_params(axis='y', left=True, right=True, **tick_params_minor)
            elif ax2_x is None and ax2_y is not None:
                # 双 Y 轴：Y只在左侧，X在上下
                preview_ax.tick_params(axis='x', top=True, bottom=True, **tick_params_major)
                preview_ax.tick_params(axis='x', top=True, bottom=True, **tick_params_minor)
                preview_ax.tick_params(axis='y', left=True, right=False, **tick_params_major)
                preview_ax.tick_params(axis='y', left=True, right=False, **tick_params_minor)
            else:
                # 双 X+Y 轴：X只在底部，Y只在左侧
                preview_ax.tick_params(axis='x', top=False, bottom=True, **tick_params_major)
                preview_ax.tick_params(axis='x', top=False, bottom=True, **tick_params_minor)
                preview_ax.tick_params(axis='y', left=True, right=False, **tick_params_major)
                preview_ax.tick_params(axis='y', left=True, right=False, **tick_params_minor)
            
            if ax2_x is not None:
                # 顶部X轴：只显示顶部X刻度
                ax2_x.tick_params(axis='x', **tick_params_major)
                ax2_x.tick_params(axis='x', **tick_params_minor)
                # 关闭Y轴刻度
                ax2_x.tick_params(axis='y', left=False, right=False, labelleft=False, labelright=False)
            
            if ax2_y is not None:
                # 右侧Y轴：只显示右侧Y刻度
                ax2_y.tick_params(axis='y', **tick_params_major)
                ax2_y.tick_params(axis='y', **tick_params_minor)
                # 关闭X轴刻度
                ax2_y.tick_params(axis='x', top=False, bottom=False, labeltop=False, labelbottom=False)
            
            # 设置刻度标签字体（tick_params已设置字号）
            for label in preview_ax.get_xticklabels() + preview_ax.get_yticklabels():
                try:
                    label.set_fontfamily(font_family)
                except Exception:
                    pass
            if ax2_x is not None:
                for label in ax2_x.get_xticklabels():
                    try:
                        label.set_fontfamily(font_family)
                    except Exception:
                        pass
            if ax2_y is not None:
                for label in ax2_y.get_yticklabels():
                    try:
                        label.set_fontfamily(font_family)
                    except Exception:
                        pass

            # 应用Major刻度设置（X轴在弧度模式下跳过，以免与π刻度冲突）
            from matplotlib.ticker import MultipleLocator
            # X轴major刻度
            mx_txt = self.settings_state.get('major_x_interval', '').strip()
            if mx_txt and (not self.settings_state.get('radian_mode', False)):
                try:
                    x_major = _parse_float(mx_txt)
                    if x_major is not None and x_major > 0:
                        preview_ax.xaxis.set_major_locator(MultipleLocator(x_major))
                except ValueError:
                    pass
            # Y轴major刻度
            my_txt = self.settings_state.get('major_y_interval', '').strip()
            if my_txt:
                try:
                    y_major = _parse_float(my_txt)
                    if y_major is not None and y_major > 0:
                        preview_ax.yaxis.set_major_locator(MultipleLocator(y_major))
                except ValueError:
                    pass
            
            # 弧度(π)刻度 - 需要在副刻度之前设置！
            # 先存储π刻度的参数，后面在draw后重新应用
            radian_tick_info = None
            if self.settings_state['radian_mode']:
                try:
                    from matplotlib.ticker import FuncFormatter, FixedLocator
                    from fractions import Fraction
                    
                    x_min, x_max = preview_ax.get_xlim()
                    print(f"[DEBUG] 弧度模式启用，X范围: {x_min} to {x_max}")
                    
                    # 确保有有效的范围
                    if x_min >= x_max:
                        x_min, x_max = 0, 2 * np.pi
                    
                    pi_min = x_min / np.pi
                    pi_max = x_max / np.pi
                    print(f"[DEBUG] π范围: {pi_min} to {pi_max}")
                    
                    # 智能选择刻度间隔
                    range_pi = pi_max - pi_min
                    if range_pi > 8:
                        step = 1.0  # π
                    elif range_pi > 4:
                        step = 0.5  # π/2
                    elif range_pi > 2:
                        step = 0.25  # π/4
                    else:
                        step = 0.125  # π/8
                    
                    print(f"[DEBUG] 刻度间隔: {step}π")
                    
                    # 生成刻度值（以π的倍数表示）
                    p = np.ceil(pi_min / step) * step
                    ticks_pi = []
                    while p <= pi_max + 0.01:
                        ticks_pi.append(p)
                        p += step
                    
                    # 转换为实际坐标值
                    x_ticks = [t * np.pi for t in ticks_pi]
                    print(f"[DEBUG] 生成的刻度值数量: {len(x_ticks)}")
                    print(f"[DEBUG] 刻度值: {x_ticks}")
                    
                    # π刻度格式化函数
                    def fmt_pi(v, pos=None):
                        if abs(v) < 1e-9:
                            return '0'
                        n = v / np.pi
                        
                        # 检查是否启用 LaTeX
                        use_latex = self.settings_state.get('use_latex', False)
                        
                        # 尝试转换为分数
                        try:
                            frac = Fraction(n).limit_denominator(12)
                            num, den = frac.numerator, frac.denominator
                            
                            # 验证分数的准确性
                            if abs(num / den - n) < 1e-9:
                                if den == 1:
                                    if num == 0:
                                        return '0'
                                    elif num == 1:
                                        pi_symbol = r'$\pi$' if use_latex else 'π'
                                        return pi_symbol
                                    elif num == -1:
                                        pi_symbol = r'$\pi$' if use_latex else 'π'
                                        return f'-{pi_symbol}'
                                    else:
                                        pi_symbol = r'$\pi$' if use_latex else 'π'
                                        return rf'${num}\pi$' if use_latex else f'{num}π'
                                else:
                                    if num == 1:
                                        return rf'$\pi/{den}$' if use_latex else f'π/{den}'
                                    elif num == -1:
                                        return rf'$-\pi/{den}$' if use_latex else f'-π/{den}'
                                    else:
                                        return rf'${num}\pi/{den}$' if use_latex else f'{num}π/{den}'
                        except Exception:
                            pass
                        
                        # 如果分数转换失败，显示小数倍数
                        return rf'${n:.2g}\pi$' if use_latex else f'{n:.2g}π'
                    
                    # 存储π刻度信息，稍后应用
                    radian_tick_info = (x_ticks, fmt_pi)
                    
                    # 应用π刻度：使用FixedLocator固定刻度位置，FuncFormatter格式化标签
                    preview_ax.xaxis.set_major_locator(FixedLocator(x_ticks))
                    preview_ax.xaxis.set_major_formatter(FuncFormatter(fmt_pi))
                    print("[DEBUG] π刻度已应用")
                    
                except Exception as e:
                    print(f"π刻度设置失败: {e}")
                    import traceback
                    traceback.print_exc()
            
            # 副刻度设置（支持自定义间隔；开启/关闭）- 放在π刻度之后
            try:
                if self.settings_state.get('minor_ticks', False):
                    preview_ax.minorticks_on()
                    mxn_txt = self.settings_state.get('minor_x_interval', '').strip()
                    if mxn_txt and not self.settings_state['radian_mode']:  # 弧度模式下跳过副刻度X轴设置
                        x_minor = _parse_float(mxn_txt)
                        if x_minor is not None and x_minor > 0:
                            preview_ax.xaxis.set_minor_locator(MultipleLocator(x_minor))
                    myn_txt = self.settings_state.get('minor_y_interval', '').strip()
                    if myn_txt:
                        y_minor = _parse_float(myn_txt)
                        if y_minor is not None and y_minor > 0:
                            preview_ax.yaxis.set_minor_locator(MultipleLocator(y_minor))
                else:
                    preview_ax.minorticks_off()
            except Exception:
                pass

            # 自动调整布局，避免标签被裁剪
            try:
                preview_fig.tight_layout()
            except Exception:
                pass
            
            # 在draw前重新应用π刻度（防止被其他操作覆盖）
            if radian_tick_info is not None:
                try:
                    x_ticks, fmt_pi = radian_tick_info
                    from matplotlib.ticker import FuncFormatter, FixedLocator
                    preview_ax.xaxis.set_major_locator(FixedLocator(x_ticks))
                    preview_ax.xaxis.set_major_formatter(FuncFormatter(fmt_pi))
                    print("[DEBUG] π刻度已在tight_layout后重新应用")
                except Exception as e:
                    print(f"重新应用π刻度失败: {e}")
            
            # 同步绘制 canvas
            preview_canvas.draw()
            
            # 打印 axes 在 figure 中的位置
            pos = preview_ax.get_position()
            print(f"[DEBUG] Axes 位置: left={pos.x0:.3f}, bottom={pos.y0:.3f}, right={pos.x1:.3f}, top={pos.y1:.3f}")
            print(f"[DEBUG] 间距: left={pos.x0:.3f}, bottom={pos.y0:.3f}, right={1-pos.x1:.3f}, top={1-pos.y1:.3f}")

        def export_image():
            path, _ = QFileDialog.getSaveFileName(dlg, "导出图片", "", "PNG (*.png);;PDF (*.pdf);;SVG (*.svg)")
            if not path:
                return
            try:
                # 在出版上下文中导出：根据字体选择启用对应的 LaTeX 配置
                font_family = self.settings_state.get('fontfamily', 'Times New Roman')
                with _publish_rc_context(font_family=font_family):
                    # 强制重新渲染以应用LaTeX设置
                    preview_canvas.draw()
                    preview_fig.savefig(path, dpi=300)
                QMessageBox.information(dlg, "成功", f"图片已导出: {path}")
            except Exception as e:
                QMessageBox.critical(dlg, "失败", f"导出失败: {e}")

        # 底部操作按钮
        row_ops = QHBoxLayout()
        btn_export = QPushButton("导出图片…")
        btn_close = QPushButton("关闭")
        btn_export.clicked.connect(export_image)
        btn_close.clicked.connect(dlg.close)
        row_ops.addWidget(btn_export)
        row_ops.addStretch()
        row_ops.addWidget(btn_close)
        root.addLayout(row_ops)

        # 初始预览
        refresh_preview()
        dlg.exec()

        # 关闭对话框后恢复主界面的 settings_state（如果存在备份）
        try:
            self.settings_state = prev_settings_state
        except Exception:
            pass

    def open_fit_dialog(self):
        """打开拟合曲线对话框"""
        if not self.loaded_files:
            QMessageBox.warning(self, "提示", "请先加载数据")
            return
        
        xcol = self.combo_x.currentText()
        ycol = self.combo_y.currentText()
        if not xcol or not ycol:
            QMessageBox.warning(self, "提示", "请选择 X 列和 Y 列")
            return

        dlg = QDialog(self)
        dlg.setWindowTitle("拟合曲线")
        dlg.resize(500, 400)
        layout = QVBoxLayout(dlg)
        layout.setSpacing(12)

        # 数据源选择（默认合并全部数据）
        src_layout = QHBoxLayout()
        src_layout.addWidget(QLabel("拟合数据源:"))
        data_combo = QComboBox()
        data_combo.addItem("全部数据 (合并)", userData=None)
        for idx, (file_path, _df) in enumerate(self.loaded_files):
            data_combo.addItem(f"数据集 {idx+1}: {os.path.basename(file_path)}", userData=idx)
        src_layout.addWidget(data_combo)
        src_layout.addStretch()
        layout.addLayout(src_layout)

        # X/Y 过滤范围（可选）
        range_layout = QHBoxLayout()
        range_layout.addWidget(QLabel("X 范围:"))
        x_min_fit = QLineEdit()
        x_min_fit.setPlaceholderText("最小值，可留空")
        x_max_fit = QLineEdit()
        x_max_fit.setPlaceholderText("最大值，可留空")
        range_layout.addWidget(x_min_fit)
        range_layout.addWidget(QLabel("~"))
        range_layout.addWidget(x_max_fit)
        range_layout.addSpacing(12)
        range_layout.addWidget(QLabel("Y 范围:"))
        y_min_fit = QLineEdit()
        y_min_fit.setPlaceholderText("最小值，可留空")
        y_max_fit = QLineEdit()
        y_max_fit.setPlaceholderText("最大值，可留空")
        range_layout.addWidget(y_min_fit)
        range_layout.addWidget(QLabel("~"))
        range_layout.addWidget(y_max_fit)
        range_layout.addStretch()
        layout.addLayout(range_layout)

        # 角度/弧度转换选项
        unit_layout = QHBoxLayout()
        unit_layout.addWidget(QLabel("X 坐标单位转换:"))
        unit_combo = QComboBox()
        unit_combo.addItems(["不转换", "角度→弧度 (×π/180)", "弧度→角度 (×180/π)"])
        unit_layout.addWidget(unit_combo)
        unit_layout.addStretch()
        layout.addLayout(unit_layout)

        # 自动检测列名中的角度/弧度关键词
        xcol_lower = xcol.lower()
        if any(kw in xcol_lower for kw in ['degree', 'deg', '°', '度']):
            unit_combo.setCurrentText("角度→弧度 (×π/180)")
        elif any(kw in xcol_lower for kw in ['radian', 'rad', '弧度']):
            # 默认不转换，但用户可以手动选择
            pass

        # 拟合类型选择
        type_layout = QHBoxLayout()
        type_layout.addWidget(QLabel("拟合类型:"))
        fit_type = QComboBox()
        fit_type.addItems(["多项式", "指数 (y=a*exp(b*x))", "对数 (y=a*log(x)+b)", "幂函数 (y=a*x^b)", "自定义函数"])
        type_layout.addWidget(fit_type)
        type_layout.addStretch()
        layout.addLayout(type_layout)

        # 多项式阶数
        degree_layout = QHBoxLayout()
        degree_layout.addWidget(QLabel("多项式阶数:"))
        degree_spin = QSpinBox()
        degree_spin.setRange(1, 10)
        degree_spin.setValue(2)
        degree_layout.addWidget(degree_spin)
        degree_layout.addStretch()
        layout.addLayout(degree_layout)

        # 自定义函数输入区
        custom_container = QWidget()
        custom_container.setVisible(False)
        custom_layout = QVBoxLayout(custom_container)
        custom_layout.setContentsMargins(0, 0, 0, 0)
        
        custom_help = QLabel("输入函数表达式 f(x, a, b, c, ...)，支持: + - * / ^ sin cos tan exp log sqrt abs, 常量: pi e")
        custom_help.setWordWrap(True)
        custom_layout.addWidget(custom_help)
        
        func_expr_layout = QHBoxLayout()
        func_expr_layout.addWidget(QLabel("f(x, params):"))
        custom_func_edit = QLineEdit("a * sin(b * x + c)")
        func_expr_layout.addWidget(custom_func_edit)
        custom_layout.addLayout(func_expr_layout)
        
        init_params_layout = QHBoxLayout()
        init_params_layout.addWidget(QLabel("初始参数值:"))
        init_params_edit = QLineEdit("1, 1, 0")
        init_params_edit.setPlaceholderText("用逗号分隔，如: 1, 1, 0")
        init_params_layout.addWidget(init_params_edit)
        custom_layout.addLayout(init_params_layout)
        
        layout.addWidget(custom_container)

        # 结果显示
        result_label = QLabel("拟合结果将在这里显示")
        result_label.setWordWrap(True)
        result_label.setStyleSheet("background-color: #2b3036; color: #ffffff; padding: 10px; border: 1px solid #424750; border-radius: 4px;")
        layout.addWidget(result_label)

        # 按钮
        btn_layout = QHBoxLayout()
        btn_fit = QPushButton("执行拟合")
        btn_apply = QPushButton("应用到图上")
        btn_apply.setEnabled(False)
        btn_close = QPushButton("关闭")
        btn_layout.addWidget(btn_fit)
        btn_layout.addWidget(btn_apply)
        btn_layout.addStretch()
        btn_layout.addWidget(btn_close)
        layout.addLayout(btn_layout)

        fit_result = {"params": None, "r2": None, "equation": None, "x_fit": None, "y_fit": None, "unit_convert": None}
        fit_task = {"id": None}

        def publish_fit(result, point_count, unit_convert):
            fit_task["id"] = None
            try:
                fit_result["equation"] = result.equation
                fit_result["r2"] = result.r2
                fit_result["x_fit"] = result.x_fit
                fit_result["y_fit"] = result.y_fit
                fit_result["unit_convert"] = unit_convert
                result_label.setText(
                    f"拟合方程：{result.equation}\n\nR² = {result.r2:.6f}\n\n点数: {point_count}"
                )
                btn_apply.setEnabled(True)
                btn_fit.setEnabled(True)
            except RuntimeError:
                # 对话框可能已在后台拟合结束前关闭。
                pass

        def show_fit_error(error, fit_method):
            fit_task["id"] = None
            LOGGER.warning("fit_failed method=%s error=%s", fit_method, error)
            if isinstance(error, FitError) and error.code == "invalid_domain" and fit_method.startswith("对数"):
                message = "对数拟合要求所有 x > 0"
            elif isinstance(error, FitError) and error.code == "invalid_domain" and fit_method.startswith("幂函数"):
                message = "幂函数拟合要求所有 x, y > 0"
            else:
                message = f"拟合过程出错: {error}"
            try:
                btn_fit.setEnabled(True)
                QMessageBox.critical(dlg, "拟合失败", message)
            except RuntimeError:
                pass

        def show_fit_cancelled():
            fit_task["id"] = None
            try:
                btn_fit.setEnabled(True)
                result_label.setText("拟合已取消")
            except RuntimeError:
                pass

        def perform_fit():
            # 解析过滤范围
            def _parse_optional_float(edit: QLineEdit, name: str):
                txt = edit.text().strip()
                if not txt:
                    return None
                try:
                    return float(txt)
                except Exception:
                    QMessageBox.warning(dlg, "提示", f"{name} 需为数字")
                    raise

            try:
                x_min_limit = _parse_optional_float(x_min_fit, "X 最小值")
                x_max_limit = _parse_optional_float(x_max_fit, "X 最大值")
                y_min_limit = _parse_optional_float(y_min_fit, "Y 最小值")
                y_max_limit = _parse_optional_float(y_max_fit, "Y 最大值")
            except Exception:
                return

            if x_min_limit is not None and x_max_limit is not None and x_max_limit <= x_min_limit:
                QMessageBox.warning(dlg, "提示", "X 最大值需大于 X 最小值")
                return
            if y_min_limit is not None and y_max_limit is not None and y_max_limit <= y_min_limit:
                QMessageBox.warning(dlg, "提示", "Y 最大值需大于 Y 最小值")
                return

            # 收集数据点
            xs_all, ys_all = [], []
            sel_idx = data_combo.currentData()
            targets = self.loaded_files if sel_idx is None else [self.loaded_files[sel_idx]]
            
            # 获取单位转换选项
            unit_convert = unit_combo.currentText()
            
            for file_path, df in targets:
                if xcol in df.columns and ycol in df.columns:
                    xs = pd.to_numeric(df[xcol], errors='coerce')
                    ys = pd.to_numeric(df[ycol], errors='coerce')
                    
                    # 应用单位转换
                    if unit_convert == "角度→弧度 (×π/180)":
                        xs = xs * np.pi / 180.0
                    elif unit_convert == "弧度→角度 (×180/π)":
                        xs = xs * 180.0 / np.pi
                    
                    valid = ~(xs.isna() | ys.isna())
                    if x_min_limit is not None:
                        valid &= xs >= x_min_limit
                    if x_max_limit is not None:
                        valid &= xs <= x_max_limit
                    if y_min_limit is not None:
                        valid &= ys >= y_min_limit
                    if y_max_limit is not None:
                        valid &= ys <= y_max_limit
                    xs_all.extend(xs[valid].tolist())
                    ys_all.extend(ys[valid].tolist())
            
            if len(xs_all) < 2:
                QMessageBox.warning(dlg, "提示", "数据点不足，无法拟合")
                return
            
            xs_all = np.array(xs_all)
            ys_all = np.array(ys_all)
            
            fit_method = fit_type.currentText()
            method_names = {
                "多项式": "polynomial",
                "指数 (y=a*exp(b*x))": "exponential",
                "对数 (y=a*log(x)+b)": "logarithmic",
                "幂函数 (y=a*x^b)": "power",
                "自定义函数": "custom",
            }
            fit_kwargs = {"degree": degree_spin.value()}
            if fit_method == "自定义函数":
                expression = custom_func_edit.text().strip()
                if not expression:
                    QMessageBox.warning(dlg, "提示", "请输入自定义函数表达式")
                    return
                try:
                    initial = [
                        float(value.strip())
                        for value in init_params_edit.text().split(",")
                        if value.strip()
                    ] or [1.0, 1.0, 1.0]
                except ValueError:
                    QMessageBox.warning(dlg, "提示", "初始参数格式错误，应为逗号分隔的数字")
                    return
                fit_kwargs.update(expression=expression, initial_parameters=initial)
            if len(xs_all) >= ASYNC_FIT_POINT_THRESHOLD:
                btn_fit.setEnabled(False)
                btn_apply.setEnabled(False)
                result_label.setText(f"正在后台拟合 {len(xs_all)} 个数据点…")
                task_kwargs = dict(fit_kwargs)
                fit_task["id"] = self.tasks.submit(
                    lambda token: fit_values(
                        xs_all,
                        ys_all,
                        method_names[fit_method],
                        cancel_check=token.raise_if_cancelled,
                        **task_kwargs,
                    ),
                    on_success=lambda result: publish_fit(result, len(xs_all), unit_convert),
                    on_failure=lambda error: show_fit_error(error, fit_method),
                    on_cancelled=show_fit_cancelled,
                )
                return

            try:
                result = fit_values(xs_all, ys_all, method_names[fit_method], **fit_kwargs)
            except FitError as error:
                show_fit_error(error, fit_method)
            else:
                publish_fit(result, len(xs_all), unit_convert)

        def apply_fit():
            if fit_result["x_fit"] is None:
                return
            
            # 清除之前的拟合线
            for line in self.fitted_lines:
                try:
                    line.remove()
                except Exception:
                    pass
            self.fitted_lines.clear()
            
            # 获取拟合数据
            x_fit_plot = fit_result["x_fit"].copy()
            y_fit_plot = fit_result["y_fit"]
            
            # 获取单位转换选项
            unit_conv = fit_result.get("unit_convert", "不转换")
            
            # 如果用户选择了角度→弧度，统一转换整个图为弧度显示
            if unit_conv == "角度→弧度 (×π/180)":
                # 设置全局单位模式为弧度
                self.x_unit_mode = 'radian'
                # 重新绘制所有原始数据（转换为弧度）
                x_col = self.combo_x.currentText()
                y_col = self.combo_y.currentText()
                self._draw_all_files(x_col, y_col, x_unit_convert='degree_to_radian')
                # 拟合曲线保持弧度（不需要反转换）
                # x_fit_plot 已经是弧度
            elif unit_conv == "弧度→角度 (×180/π)":
                # 用户选择弧度→角度，保持原样显示
                self.x_unit_mode = None
                # 拟合时已转为角度，需要转回弧度以匹配原数据
                x_fit_plot = x_fit_plot * np.pi / 180.0
            else:
                # 不转换，保持原样
                self.x_unit_mode = None
            
            # 绘制拟合曲线（确保 zorder 在上层，且带有标签）
            line, = self.ax.plot(x_fit_plot, y_fit_plot, 
                                 'r--', linewidth=2, label=f'拟合: {fit_result["equation"]}', zorder=100)
            self.fitted_lines.append(line)

            # 如果是弧度模式，设置x轴刻度为π的倍数
            if self.x_unit_mode == 'radian':
                self._set_pi_ticks()

            # 明确重建/更新图例，确保包含所有带标签的曲线（包括拟合线）
            try:
                if self.legend_config.get("show", True):
                    handles, labels = self.ax.get_legend_handles_labels()
                    # 使用用户配置的字体大小和位置
                    loc = self.legend_config.get('loc', 'best')
                    fontsize = self.legend_config.get('fontsize', 12)
                    frameon = self.legend_config.get('frameon', True)
                    framealpha = self.legend_config.get('framealpha', 0.8)
                    # 如果用户提供了具体坐标字符串（如 '0.5,0.5'），尝试解析
                    loc_text = str(self.legend_config.get('loc', 'best')).strip()
                    try:
                        if ',' in loc_text:
                            xy = tuple(float(s.strip()) for s in loc_text.split(','))
                            loc_arg = xy
                        else:
                            loc_arg = loc_text
                    except Exception:
                        loc_arg = loc

                    # 重新设置图例（使用支持中文的字体属性）
                    from matplotlib.font_manager import FontProperties
                    legend_font = self.settings_state.get('fontfamily', 'Helvetica')
                    font_prop = FontProperties(family=_font_families(legend_font), size=fontsize)
                    if handles and labels:
                        self.ax.legend(handles, labels, fontsize=fontsize, loc=loc_arg, frameon=frameon, framealpha=framealpha, prop=font_prop)
                else:
                    # 用户已关闭图例，移除现有图例
                    if self.ax.get_legend() is not None:
                        self.ax.get_legend().remove()
            except Exception:
                try:
                    from matplotlib.font_manager import FontProperties
                    legend_font = self.settings_state.get('fontfamily', 'Helvetica')
                    font_prop = FontProperties(family=_font_families(legend_font), size=12)
                    self.ax.legend(prop=font_prop)
                except Exception:
                    pass

            self.canvas.draw()
            
            QMessageBox.information(dlg, "成功", "拟合曲线已添加到图上")

        btn_fit.clicked.connect(perform_fit)
        btn_apply.clicked.connect(apply_fit)
        btn_close.clicked.connect(dlg.close)
        dlg.finished.connect(
            lambda _result: self.tasks.cancel(fit_task["id"])
            if fit_task["id"] is not None
            else None
        )
        
        # 类型切换时切换可见控件
        def on_type_change():
            current = fit_type.currentText()
            is_poly = current == "多项式"
            is_custom = current == "自定义函数"
            degree_spin.setEnabled(is_poly)
            custom_container.setVisible(is_custom)
        fit_type.currentTextChanged.connect(on_type_change)
        on_type_change()
        
        dlg.exec()

    # 拖拽事件
    def dragEnterEvent(self, event):
        if event.mimeData().hasUrls():
            event.acceptProposedAction()

    def dropEvent(self, event):
        for url in event.mimeData().urls():
            file_path = url.toLocalFile()
            if getattr(self, "placeholder_active", False):
                self.loaded_files.clear()
                self.history.reset(self.loaded_files)
                try:
                    self.combo_x.clear()
                    self.combo_y.clear()
                except Exception:
                    pass
                self.placeholder_active = False
            self.load_file_async(file_path)
    
    # 加载文件

    def load_file(self, file_path):
        """Load data through the GUI-independent M2 import boundary."""
        try:
            result = read_data_file(file_path)
        except DataIOError as error:
            self._show_import_error(error)
            return

        self._publish_import_result(file_path, result)

    def load_file_async(self, file_path):
        """Read off the GUI thread and publish the completed result on the GUI thread."""
        self.statusBar().showMessage(f"正在读取文件：{file_path}")

        def publish(result):
            self._publish_import_result(file_path, result)
            self.plot_selected()

        return self.tasks.submit(
            lambda token: read_data_file(file_path),
            on_success=publish,
            on_failure=self._show_import_error,
            on_cancelled=lambda: self.statusBar().showMessage(f"已取消文件读取：{file_path}"),
        )

    def _show_import_error(self, error):
        self.statusBar().showMessage(f"文件读取失败: {error}")
        LOGGER.error("import_failed error=%s", error)

    def _publish_import_result(self, file_path, result):
        """Publish an ImportResult; callers must run this method on the GUI thread."""

        df = result.frame
        self.loaded_files.append((file_path, df))
        self.history.reset(self.loaded_files)
        self.col_unicode_map.update({col: latex_to_unicode(str(col)) for col in df.columns})

        self.combo_x.clear()
        self.combo_y.clear()
        self.combo_x.addItems(df.columns)
        self.combo_y.addItems(df.columns)

        if not self.last_x_col:
            self.last_x_col = df.columns[0]
        if not self.last_y_col and len(df.columns) > 1:
            self.last_y_col = df.columns[1]
        self.combo_x.setCurrentText(self.last_x_col)
        self.combo_y.setCurrentText(self.last_y_col)

        self.statusBar().showMessage(
            f"已加载文件：{file_path} (编码: {result.encoding}, 分隔符: {repr(result.separator)})"
        )
        LOGGER.info(
            "import_succeeded path=%s encoding=%s separator=%r rows=%d columns=%d",
            file_path,
            result.encoding,
            result.separator,
            len(df),
            len(df.columns),
        )

    # 绘图
    def plot_selected(self):
        if not self.loaded_files:
            self.statusBar().showMessage("请先加载数据文件")
            return

        x_col = self.combo_x.currentText()
        y_col = self.combo_y.currentText()
        if not x_col or not y_col:
            self.statusBar().showMessage("请选择 X 列和 Y 列")
            return
        
        self._draw_all_files(x_col, y_col)

        # 记住列名
        self.last_x_col = x_col
        self.last_y_col = y_col

    # 清空
    def clear_plot(self):
        self.tasks.cancel_all()
        self.ax.clear()
        self.canvas.draw()
        self.loaded_files.clear()
        self.history.reset(self.loaded_files)
        # 清空下拉选择并重置记录的列
        try:
            self.combo_x.clear()
            self.combo_y.clear()
        except Exception:
            pass
        self.last_x_col = ""
        self.last_y_col = ""
        self.placeholder_active = False
        self.statusBar().showMessage("已清空图形、文件数据和 X/Y 选择")

    #对所有已加载文件的 Y 列执行去噪处理
    def apply_denoise(self):
        if not getattr(self, "loaded_files", None):
            self.statusBar().showMessage("请先加载数据文件")
            return

        y_col = self.combo_y.currentText()
        if not y_col:
            self.statusBar().showMessage("请选择 Y 列")
            return
        
        # 弹出对话框让用户选择去噪参数
        dlg = QDialog(self)
        dlg.setWindowTitle("去噪参数设置")
        dlg.resize(500, 550)
        layout = QVBoxLayout(dlg)
        
        # 窗口长度选择
        window_layout = QHBoxLayout()
        window_label = QLabel("窗口长度 (window_length)：")
        window_spin = QSpinBox()
        window_spin.setMinimum(3)
        window_spin.setMaximum(51)
        window_spin.setSingleStep(2)  # 只能选择奇数
        window_spin.setValue(self.last_denoise_window_length)
        window_layout.addWidget(window_label)
        window_layout.addWidget(window_spin)
        window_layout.addStretch()
        layout.addLayout(window_layout)
        
        # 多项式阶数选择
        poly_layout = QHBoxLayout()
        poly_label = QLabel("多项式阶数 (polyorder)：")
        poly_combo = QComboBox()
        poly_combo.addItems(["1", "2", "3", "4", "5"])
        poly_combo.setCurrentIndex(self.last_denoise_polyorder - 1)  # 根据保存值设置
        poly_layout.addWidget(poly_label)
        poly_layout.addWidget(poly_combo)
        poly_layout.addStretch()
        layout.addLayout(poly_layout)
        
        # 区间选择
        range_checkbox = QCheckBox("只处理指定区间")
        range_checkbox.setChecked(self.last_denoise_use_range)
        layout.addWidget(range_checkbox)
        
        # X 列选择
        x_col_layout = QHBoxLayout()
        x_col_label = QLabel("X 列：")
        x_col_combo = QComboBox()
        for i in range(self.combo_x.count()):
            x_col_combo.addItem(self.combo_x.itemText(i))
        # 如果上次有保存的 X 列，则设置为那个值；否则设置为当前选择
        if self.last_denoise_x_col and self.last_denoise_x_col in [self.combo_x.itemText(i) for i in range(self.combo_x.count())]:
            x_col_combo.setCurrentText(self.last_denoise_x_col)
        else:
            x_col_combo.setCurrentText(self.combo_x.currentText())
        x_col_combo.setEnabled(self.last_denoise_use_range)
        x_col_layout.addWidget(x_col_label)
        x_col_layout.addWidget(x_col_combo)
        x_col_layout.addStretch()
        layout.addLayout(x_col_layout)
        
        # 范围输入
        range_layout = QHBoxLayout()
        x1_label = QLabel("x1 (范围左界)：")
        x1_spin = QDoubleSpinBox()
        x1_spin.setMinimum(-999999)
        x1_spin.setMaximum(999999)
        x1_spin.setValue(self.last_denoise_x1 if self.last_denoise_x1 is not None else 0)
        x1_spin.setDecimals(6)
        x1_spin.setSingleStep(0.1)
        x1_spin.setEnabled(self.last_denoise_use_range)
        
        x2_label = QLabel("x2 (范围右界)：")
        x2_spin = QDoubleSpinBox()
        x2_spin.setMinimum(-999999)
        x2_spin.setMaximum(999999)
        x2_spin.setValue(self.last_denoise_x2 if self.last_denoise_x2 is not None else 10)
        x2_spin.setDecimals(6)
        x2_spin.setSingleStep(0.1)
        x2_spin.setEnabled(self.last_denoise_use_range)
        
        range_layout.addWidget(x1_label)
        range_layout.addWidget(x1_spin)
        range_layout.addStretch()
        range_layout.addWidget(x2_label)
        range_layout.addWidget(x2_spin)
        layout.addLayout(range_layout)
        
        # 连接复选框信号
        def on_range_checkbox_changed(checked):
            x_col_combo.setEnabled(checked)
            x1_spin.setEnabled(checked)
            x2_spin.setEnabled(checked)
        
        range_checkbox.stateChanged.connect(on_range_checkbox_changed)
        
        # 说明
        info_label = QLabel("选择要进行去噪的曲线：")
        layout.addWidget(info_label)
        
        # 曲线选择表格
        table, checkboxes = create_file_selection_table(self.loaded_files, name_width=400)
        
        layout.addWidget(table)
        
        # 按钮
        btn_layout, btn_ok, btn_cancel = create_dialog_buttons()
        layout.addLayout(btn_layout)
        
        dlg.setLayout(layout)
        
        def on_ok():
            window_length = window_spin.value()
            # 确保是奇数
            if window_length % 2 == 0:
                window_length += 1
            
            polyorder = int(poly_combo.currentText())
            
            # 检查是否至少选择了一个文件
            selected_count = sum(1 for cb in checkboxes if cb.isChecked())
            if selected_count == 0:
                QMessageBox.warning(dlg, "提示", "请至少选择一个曲线进行去噪")
                return
            
            # 获取区间参数
            use_range = range_checkbox.isChecked()
            x_col = x_col_combo.currentText() if use_range else None
            x1 = x1_spin.value() if use_range else None
            x2 = x2_spin.value() if use_range else None
            
            if use_range and x1 >= x2:
                QMessageBox.warning(dlg, "提示", "x1 必须小于 x2")
                return
            
            changes = []
            for i, (file_path, df) in enumerate(self.loaded_files):
                # 只处理选中的文件
                if not checkboxes[i].isChecked():
                    print(f"[denoise] skip {os.path.basename(file_path)} (未选中)")
                    continue
                
                if y_col in df.columns:
                    try:
                        # 获取当前行范围过滤的索引
                        row_positions = self._get_filtered_row_positions(df)
                        
                        # 如果有行范围过滤，只处理这些行
                        if self.row_filter_enabled and len(row_positions) < len(df):
                            # 创建要处理的数据的深拷贝
                            df_to_process = df.iloc[row_positions].copy()
                            
                            # 在拷贝上进行处理
                            if use_range and x_col and x_col in df_to_process.columns:
                                df_to_process[y_col] = denoise_data(
                                    df_to_process, 
                                    y_col=y_col, 
                                    window_length=window_length, 
                                    polyorder=polyorder,
                                    x_col=x_col,
                                    x1=x1,
                                    x2=x2
                                )
                            else:
                                df_to_process[y_col] = denoise_data(
                                    df_to_process, 
                                    y_col=y_col, 
                                    window_length=window_length, 
                                    polyorder=polyorder
                                )
                            
                            values = df_to_process[y_col].to_numpy(copy=True)
                            print(f"[denoise] applied to {os.path.basename(file_path)} ({y_col}, rows {len(row_positions)}/{len(df)}), window={window_length}, polyorder={polyorder}")
                        else:
                            # 如果没有行范围过滤，直接处理整个 df
                            if use_range and x_col and x_col in df.columns:
                                values = denoise_data(
                                    df, 
                                    y_col=y_col, 
                                    window_length=window_length, 
                                    polyorder=polyorder,
                                    x_col=x_col,
                                    x1=x1,
                                    x2=x2
                                )
                                print(f"[denoise] applied to {os.path.basename(file_path)} ({y_col}), window={window_length}, polyorder={polyorder}, range=[{x1}, {x2}]")
                            else:
                                values = denoise_data(df, y_col=y_col, window_length=window_length, polyorder=polyorder)
                                print(f"[denoise] applied to {os.path.basename(file_path)} ({y_col}), window={window_length}, polyorder={polyorder}")
                            row_positions = list(range(len(df)))
                        changes.append((i, y_col, row_positions, values))
                    except Exception as e:
                        print(f"[denoise] failed on {file_path}: {e}")
                else:
                    print(f"[denoise] skip {os.path.basename(file_path)}: no column {y_col}")
            
            changed = self._commit_column_changes(changes)
            if changed:
                if use_range:
                    self.statusBar().showMessage(f"去噪完成（列: {y_col}, 窗口: {window_length}, 阶数: {polyorder}, 区间: [{x1}, {x2}]）")
                else:
                    self.statusBar().showMessage(f"去噪完成（列: {y_col}, 窗口: {window_length}, 阶数: {polyorder})")
                
                # 保存参数以供下次使用
                self.last_denoise_window_length = window_length
                self.last_denoise_polyorder = polyorder
                self.last_denoise_use_range = use_range
                self.last_denoise_x_col = x_col if use_range else None
                self.last_denoise_x1 = x1 if use_range else 0
                self.last_denoise_x2 = x2 if use_range else 10
                
                self.replot_all()
            else:
                self.statusBar().showMessage("没有文件包含所选 Y 列，未做处理")
            
            dlg.accept()
        
        def on_cancel():
            dlg.reject()
        
        btn_ok.clicked.connect(on_ok)
        btn_cancel.clicked.connect(on_cancel)
        
        dlg.exec()

    #对所有已加载文件的 Y 列执行局部去趋势处理
    def apply_local_detrend(self):
        if not getattr(self, "loaded_files", None):
            self.statusBar().showMessage("请先加载数据文件")
            return

        x_col = self.combo_x.currentText()
        y_col = self.combo_y.currentText()
        
        if not x_col or not y_col:
            self.statusBar().showMessage("请选择 X/Y 列")
            return
        
        # 弹出对话框让用户设置参数
        dlg = QDialog(self)
        dlg.setWindowTitle("局部平坦化处理")
        dlg.resize(550, 550)
        layout = QVBoxLayout(dlg)
        
        # X 范围选择
        x_range_label = QLabel("设置处理范围：")
        layout.addWidget(x_range_label)
        
        x_range_layout = QHBoxLayout()
        x1_label = QLabel("x1 (范围左界)：")
        x1_spin = QDoubleSpinBox()
        x1_spin.setMinimum(-999999)
        x1_spin.setMaximum(999999)
        x1_spin.setValue(0)
        x1_spin.setDecimals(6)
        x1_spin.setSingleStep(0.1)
        
        x2_label = QLabel("x2 (范围右界)：")
        x2_spin = QDoubleSpinBox()
        x2_spin.setMinimum(-999999)
        x2_spin.setMaximum(999999)
        x2_spin.setValue(10)
        x2_spin.setDecimals(6)
        x2_spin.setSingleStep(0.1)
        
        x_range_layout.addWidget(x1_label)
        x_range_layout.addWidget(x1_spin)
        x_range_layout.addStretch()
        x_range_layout.addWidget(x2_label)
        x_range_layout.addWidget(x2_spin)
        layout.addLayout(x_range_layout)
        
        # 过渡宽度选择
        trans_layout = QHBoxLayout()
        trans_label = QLabel("过渡宽度 (transition)：")
        trans_spin = QDoubleSpinBox()
        trans_spin.setMinimum(0)
        trans_spin.setMaximum(99999)
        trans_spin.setValue(0)
        trans_spin.setDecimals(2)
        trans_spin.setSingleStep(10)
        trans_layout.addWidget(trans_label)
        trans_layout.addWidget(trans_spin)
        trans_layout.addStretch()
        layout.addLayout(trans_layout)
        
        # 锚点选择
        anchor_layout = QHBoxLayout()
        anchor_label = QLabel("锚点位置 (anchor)：")
        anchor_combo = QComboBox()
        anchor_combo.addItems(["left (左边界固定)", "right (右边界固定)", "center (中心固定)"])
        anchor_combo.setCurrentIndex(0)
        anchor_layout.addWidget(anchor_label)
        anchor_layout.addWidget(anchor_combo)
        anchor_layout.addStretch()
        layout.addLayout(anchor_layout)
        
        # 强度选择
        strength_layout = QHBoxLayout()
        strength_label = QLabel("处理强度 (strength)：")
        strength_spin = QDoubleSpinBox()
        strength_spin.setMinimum(0)
        strength_spin.setMaximum(1)
        strength_spin.setValue(1.0)
        strength_spin.setDecimals(2)
        strength_spin.setSingleStep(0.1)
        strength_layout.addWidget(strength_label)
        strength_layout.addWidget(strength_spin)
        strength_layout.addStretch()
        layout.addLayout(strength_layout)
        
        # 说明
        info_label = QLabel("说明：锚点固定该端不动；强度控制处理的程度（0=无处理，1=完全处理）")
        info_label.setStyleSheet("color: #666666; font-size: 11px;")
        layout.addWidget(info_label)
        
        # 曲线选择表格
        curve_label = QLabel("选择要处理的曲线：")
        layout.addWidget(curve_label)
        
        table, checkboxes = create_file_selection_table(self.loaded_files, name_width=450)
        
        layout.addWidget(table)
        
        # 按钮
        btn_layout, btn_ok, btn_cancel = create_dialog_buttons()
        layout.addLayout(btn_layout)
        
        dlg.setLayout(layout)
        
        def on_ok():
            x1 = x1_spin.value()
            x2 = x2_spin.value()
            transition = trans_spin.value()
            strength = strength_spin.value()
            anchor_text = anchor_combo.currentText()
            anchor = anchor_text.split()[0].lower()  # 提取 'left', 'right' 或 'center'
            
            if x1 >= x2:
                QMessageBox.warning(dlg, "提示", "x1 必须小于 x2")
                return
            
            selected_count = sum(1 for cb in checkboxes if cb.isChecked())
            if selected_count == 0:
                QMessageBox.warning(dlg, "提示", "请至少选择一个曲线进行处理")
                return
            
            changes = []
            for i, (file_path, df) in enumerate(self.loaded_files):
                if not checkboxes[i].isChecked():
                    print(f"[local_flatten] skip {os.path.basename(file_path)} (未选中)")
                    continue
                
                if x_col not in df.columns or y_col not in df.columns:
                    print(f"[local_flatten] skip {os.path.basename(file_path)}: missing column {x_col} or {y_col}")
                    continue
                
                try:
                    # 获取当前行范围过滤的索引
                    row_positions = self._get_filtered_row_positions(df)
                    
                    # 如果有行范围过滤，只处理这些行
                    if self.row_filter_enabled and len(row_positions) < len(df):
                        # 创建要处理的数据的深拷贝
                        df_to_process = df.iloc[row_positions].copy()
                        
                        # 在拷贝上进行处理
                        df_to_process[y_col] = local_flatten_keep_anchor(
                            df_to_process[x_col].values,
                            df_to_process[y_col].values,
                            x1, x2,
                            transition=transition,
                            anchor=anchor,
                            strength=strength
                        )
                        
                        values = df_to_process[y_col].to_numpy(copy=True)
                        print(f"[local_flatten] applied to {os.path.basename(file_path)} (rows {len(row_positions)}/{len(df)}), x1={x1}, x2={x2}, transition={transition}, anchor={anchor}, strength={strength}")
                    else:
                        # 如果没有行范围过滤，直接处理整个 df
                        values = local_flatten_keep_anchor(
                            df[x_col].values,
                            df[y_col].values,
                            x1, x2,
                            transition=transition,
                            anchor=anchor,
                            strength=strength
                        )
                        print(f"[local_flatten] applied to {os.path.basename(file_path)}, x1={x1}, x2={x2}, transition={transition}, anchor={anchor}, strength={strength}")
                        row_positions = list(range(len(df)))
                    changes.append((i, y_col, row_positions, values))
                except Exception as e:
                    print(f"[local_flatten] failed on {file_path}: {e}")
            
            changed = self._commit_column_changes(changes)
            if changed:
                self.statusBar().showMessage(f"局部处理完成 (x1={x1}, x2={x2}, transition={transition}, anchor={anchor}, strength={strength})")
                self.replot_all()
            else:
                self.statusBar().showMessage("没有文件进行处理")
            
            dlg.accept()
        
        def on_cancel():
            dlg.reject()
        
        btn_ok.clicked.connect(on_ok)
        btn_cancel.clicked.connect(on_cancel)
        
        dlg.exec()

    def open_row_filter_dialog(self):
        """打开行范围选择对话框"""
        if not getattr(self, "loaded_files", None):
            self.statusBar().showMessage("请先加载数据文件")
            return

        dlg = QDialog(self)
        dlg.setWindowTitle("选择显示数据范围")
        dlg.resize(450, 350)
        layout = QVBoxLayout(dlg)
        
        # 说明文本
        info_label = QLabel("选择要显示的数据范围：")
        layout.addWidget(info_label)
        
        # 选项组
        group_box = QGroupBox("显示模式")
        group_layout = QVBoxLayout()
        
        radio_all = QRadioButton("显示所有数据")
        radio_first_half = QRadioButton("显示前 1/2 的数据")
        radio_second_half = QRadioButton("显示后 1/2 的数据")
        radio_custom = QRadioButton("自定义范围（Python 切片语法）")
        
        # 根据当前状态设置选中
        if not self.row_filter_enabled or self.row_filter_mode == 'all':
            radio_all.setChecked(True)
        elif self.row_filter_mode == 'first_half':
            radio_first_half.setChecked(True)
        elif self.row_filter_mode == 'second_half':
            radio_second_half.setChecked(True)
        elif self.row_filter_mode == 'custom':
            radio_custom.setChecked(True)
        
        group_layout.addWidget(radio_all)
        group_layout.addWidget(radio_first_half)
        group_layout.addWidget(radio_second_half)
        group_layout.addWidget(radio_custom)
        group_box.setLayout(group_layout)
        layout.addWidget(group_box)
        
        # 自定义范围输入框
        custom_layout = QHBoxLayout()
        custom_label = QLabel("切片表达式：")
        custom_input = QLineEdit()
        custom_input.setText(self.row_filter_custom_slice if self.row_filter_custom_slice else ":10")
        custom_input.setPlaceholderText("例如: :10 (前10行), 10: (从第10行开始), 10:20 (第10到20行)")
        custom_input.setEnabled(radio_custom.isChecked())
        
        custom_layout.addWidget(custom_label)
        custom_layout.addWidget(custom_input)
        layout.addLayout(custom_layout)
        
        # 提示信息
        hint_label = QLabel("Python 切片语法示例：")
        hint_text = QLabel("  ':'  表示所有行\n"
                           "  ':10'  表示前 10 行\n"
                           "  '10:'  表示从第 10 行开始\n"
                           "  '10:20'  表示第 10 到 20 行\n"
                           "  '::2'  表示每隔 1 行取 1 个")
        hint_text.setStyleSheet("color: #666; font-size: 11px;")
        layout.addWidget(hint_label)
        layout.addWidget(hint_text)
        
        # 信息标签：显示当前数据行数
        info_rows_label = QLabel()
        if self.loaded_files:
            total_rows = len(self.loaded_files[0][1]) if self.loaded_files else 0
            info_rows_label.setText(f"当前数据总行数: {total_rows}")
        layout.addWidget(info_rows_label)
        
        layout.addStretch()
        
        # 连接单选按钮信号
        def on_radio_changed():
            custom_input.setEnabled(radio_custom.isChecked())
        
        radio_all.toggled.connect(on_radio_changed)
        radio_first_half.toggled.connect(on_radio_changed)
        radio_second_half.toggled.connect(on_radio_changed)
        radio_custom.toggled.connect(on_radio_changed)
        
        # 按钮
        btn_layout = QHBoxLayout()
        btn_ok = QPushButton("确定")
        btn_cancel = QPushButton("取消")
        btn_layout.addWidget(btn_ok)
        btn_layout.addWidget(btn_cancel)
        layout.addLayout(btn_layout)
        
        def on_ok():
            if radio_all.isChecked():
                self.row_filter_enabled = False
                self.row_filter_mode = 'all'
                self.statusBar().showMessage("已设置为显示所有数据")
            elif radio_first_half.isChecked():
                self.row_filter_enabled = True
                self.row_filter_mode = 'first_half'
                self.statusBar().showMessage("已设置为显示前 1/2 的数据")
            elif radio_second_half.isChecked():
                self.row_filter_enabled = True
                self.row_filter_mode = 'second_half'
                self.statusBar().showMessage("已设置为显示后 1/2 的数据")
            elif radio_custom.isChecked():
                self.row_filter_enabled = True
                self.row_filter_mode = 'custom'
                self.row_filter_custom_slice = custom_input.text().strip()
                if not self.row_filter_custom_slice:
                    QMessageBox.warning(dlg, "提示", "请输入有效的切片表达式")
                    return
                self.statusBar().showMessage(f"已设置为自定义范围: {self.row_filter_custom_slice}")
            
            self.replot_all()
            dlg.accept()
        
        def on_cancel():
            dlg.reject()
        
        btn_ok.clicked.connect(on_ok)
        btn_cancel.clicked.connect(on_cancel)
        
        dlg.exec()

    def _get_filtered_rows(self, df):
        """根据行范围过滤参数获取过滤后的行索引"""
        if not self.row_filter_enabled:
            return df.index
        
        total_rows = len(df)
        
        if self.row_filter_mode == 'first_half':
            # 显示前 1/2
            max_rows = total_rows // 2
            return df.index[:max_rows]
        elif self.row_filter_mode == 'second_half':
            # 显示后 1/2
            start_rows = total_rows // 2
            return df.index[start_rows:]
        elif self.row_filter_mode == 'custom':
            # 使用 Python 切片语法
            try:
                if self.row_filter_custom_slice is None:
                    return df.index
                
                # 解析切片字符串（支持 Python 切片语法）
                # 例如: ":10", "10:", "10:20", ":", "::"
                slice_str = self.row_filter_custom_slice.strip()
                
                # 使用 eval 解析切片（在受控环境中是安全的）
                # 构造一个临时数据来测试切片
                temp_list = list(range(total_rows))
                try:
                    # 解析切片表达式
                    sliced_indices = eval(f"temp_list[{slice_str}]")
                    if isinstance(sliced_indices, list):
                        return df.index[sliced_indices]
                    else:
                        # 如果是单个数字，返回该行
                        return df.index[[sliced_indices]]
                except Exception as e:
                    print(f"[row_filter] 切片语法错误: {e}")
                    return df.index
            except Exception as e:
                print(f"[row_filter] 解析切片失败: {e}")
                return df.index
        else:
            return df.index

    def _get_filtered_row_positions(self, df):
        total_rows = len(df)
        if not self.row_filter_enabled or self.row_filter_mode == 'all':
            return list(range(total_rows))
        if self.row_filter_mode == 'first_half':
            return list(range(total_rows // 2))
        if self.row_filter_mode == 'second_half':
            return list(range(total_rows // 2, total_rows))
        if self.row_filter_mode == 'custom':
            try:
                temp_list = list(range(total_rows))
                selected = eval(f"temp_list[{(self.row_filter_custom_slice or ':').strip()}]")
                return selected if isinstance(selected, list) else [selected]
            except Exception as error:
                print(f"[row_filter] 切片语法错误: {error}")
        return list(range(total_rows))

    #对所有已加载文件的 Y 列执行纵向对称处理
    def apply_center(self):
        if not getattr(self, "loaded_files", None):
            self.statusBar().showMessage("请先加载数据文件")
            return

        y_col = self.combo_y.currentText()
        if not y_col:
            self.statusBar().showMessage("请选择 Y 列")
            return
        
        changes = []
        for file_position, (file_path, df) in enumerate(self.loaded_files):
            if y_col in df.columns:
                try:
                    # 获取当前行范围过滤的索引
                    row_positions = self._get_filtered_row_positions(df)
                    
                    # 如果有行范围过滤，只处理这些行
                    if self.row_filter_enabled and len(row_positions) < len(df):
                        df_to_process = df.iloc[row_positions].copy()
                        values = center_data(df_to_process[y_col])
                        print(f"[center] applied to {os.path.basename(file_path)} ({y_col}, rows {len(row_positions)}/{len(df)})")
                    else:
                        values = center_data(df[y_col])
                        row_positions = list(range(len(df)))
                        print(f"[center] applied to {os.path.basename(file_path)} ({y_col})")
                    changes.append((file_position, y_col, row_positions, values))
                except Exception as e:
                    print(f"[center] failed on {file_path}: {e}")
            else:
                print(f"[center] skip {os.path.basename(file_path)}: no column {y_col}")

        changed = self._commit_column_changes(changes)
        if changed:
            self.statusBar().showMessage(f"对称处理完成（列: {y_col})")
            self.replot_all()
        else:
            self.statusBar().showMessage("没有文件包含所选 Y 列，未做处理")

    #对所有已加载文件的 Y 列执行归一化处理
    def apply_normalize(self):
        if not getattr(self, "loaded_files", None):
            self.statusBar().showMessage("请先加载数据文件")
            return

        y_col = self.combo_y.currentText()
        if not y_col:
            self.statusBar().showMessage("请选择 Y 列")
            return
        
        changes = []
        for file_position, (file_path, df) in enumerate(self.loaded_files):
            if y_col in df.columns:
                try:
                    # 获取当前行范围过滤的索引
                    row_positions = self._get_filtered_row_positions(df)
                    
                    # 如果有行范围过滤，只处理这些行
                    if self.row_filter_enabled and len(row_positions) < len(df):
                        df_to_process = df.iloc[row_positions].copy()
                        # 先对称
                        df_to_process[y_col] = center_data(df_to_process[y_col])
                        # 再归一化
                        normalized_Y, top_n_avg = normalize_data(df_to_process[y_col])
                        values = normalized_Y
                        print(f"[normalize] applied to {os.path.basename(file_path)} ({y_col}, rows {len(row_positions)}/{len(df)}), top_n_avg={top_n_avg}")
                    else:
                        # 先对称
                        centered = center_data(df[y_col])
                        # 再归一化
                        normalized_Y, top_n_avg = normalize_data(centered)
                        values = normalized_Y
                        row_positions = list(range(len(df)))
                        print(f"[normalize] applied to {os.path.basename(file_path)} ({y_col}), top_n_avg={top_n_avg}")
                    changes.append((file_position, y_col, row_positions, values))
                except Exception as e:
                    print(f"[normalize] failed on {file_path}: {e}")
            else:
                print(f"[normalize] skip {os.path.basename(file_path)}: no column {y_col}")

        changed = self._commit_column_changes(changes)
        if changed:
            self.statusBar().showMessage(f"归一化完成（列: {y_col})")
            self.replot_all()
        else:
            self.statusBar().showMessage("没有文件包含所选 Y 列，未做处理")

    #对已加载的所有文件进行背景信号去除（多项式拟合）
    def remove_background(self):
        if not self.loaded_files:
            self.statusBar().showMessage("请先加载数据文件")
            return

        x_col = self.combo_x.currentText()
        y_col = self.combo_y.currentText()

        if not x_col or not y_col:
            self.statusBar().showMessage("请先选择 X/Y 列")
            return
        
        # 弹出表格让用户输入每条曲线的区间
        dlg = QDialog(self)
        dlg.setWindowTitle("设置去多项式背景拟合区间")
        dlg.resize(450, 450)
        layout = QVBoxLayout(dlg)

        # 添加多项式阶数选择
        order_layout = QHBoxLayout()
        order_label = QLabel("多项式阶数：")
        order_combo = QComboBox()
        order_combo.addItems(["1 (线性)", "2 (二次)", "3 (三次)", "4 (四次)", "5 (五次)"])
        order_combo.setCurrentIndex(0)  # 默认线性
        order_layout.addWidget(order_label)
        order_layout.addWidget(order_combo)
        order_layout.addStretch()
        layout.addLayout(order_layout)

        label = QLabel("单位请与数据列一致，留空表示跳过该曲线")
        layout.addWidget(label)

        table = QTableWidget(dlg)
        table.setRowCount(len(self.loaded_files))
        table.setColumnCount(3)
        table.setHorizontalHeaderLabels(["文件名", "x_min", "x_max"])

        for i, (path, df) in enumerate(self.loaded_files):
            table.setItem(i, 0, QTableWidgetItem(os.path.basename(path)))
            table.setItem(i, 1, QTableWidgetItem(""))  # 默认空
            table.setItem(i, 2, QTableWidgetItem(""))

        layout.addWidget(table)

        btn_layout = QHBoxLayout()
        btn_ok = QPushButton("确定")
        btn_cancel = QPushButton("取消")
        btn_layout.addWidget(btn_ok)
        btn_layout.addWidget(btn_cancel)
        layout.addLayout(btn_layout)

        dlg.setLayout(layout)

        def on_ok():
            # 获取用户选择的多项式阶数
            poly_order = int(order_combo.currentText().split()[0])
            
            success_count = 0
            skip_count = 0
            error_count = 0
            changes = []
            
            for i, (path, df) in enumerate(self.loaded_files):
                item_min = table.item(i, 1)
                item_max = table.item(i, 2)
                
                # 检查是否留空（跳过）
                if not item_min or not item_max or not item_min.text().strip() or not item_max.text().strip():
                    skip_count += 1
                    continue
                
                try:
                    B_min = float(item_min.text())
                    B_max = float(item_max.text())
                    
                    if x_col not in df.columns or y_col not in df.columns:
                        print(f"[background] 警告: {os.path.basename(path)} 缺少列 {x_col} 或 {y_col}")
                        error_count += 1
                        continue
                    
                    # 获取当前行范围过滤的索引
                    row_positions = self._get_filtered_row_positions(df)
                    
                    # 如果有行范围过滤，只处理这些行
                    if self.row_filter_enabled and len(row_positions) < len(df):
                        df_to_process = df.iloc[row_positions].copy()
                        # 选择指定区间的数据（在行范围内）
                        mask = (df_to_process[x_col] >= B_min) & (df_to_process[x_col] <= B_max)
                        
                        if not mask.any():
                            print(f"[background] 警告: {os.path.basename(path)} 在区间 [{B_min}, {B_max}] 没有数据点")
                            error_count += 1
                            continue
                        
                        # 检查数据点数量是否足够
                        n_points = mask.sum()
                        if n_points <= poly_order:
                            print(f"[background] 警告: {os.path.basename(path)} 数据点({n_points})不足以拟合{poly_order}次多项式(需要>{poly_order})")
                            error_count += 1
                            continue
                        
                        # 使用纯处理核心拟合并移除背景
                        background_result = remove_polynomial_background(
                            df_to_process[x_col], df_to_process[y_col], B_min, B_max, poly_order
                        )
                        values = background_result.values
                        print(f"[background] 成功: 去{poly_order}次多项式背景 - {os.path.basename(path)} ({y_col}, rows {len(row_positions)}/{len(df)})")
                    else:
                        # 选择指定区间的数据
                        mask = (df[x_col] >= B_min) & (df[x_col] <= B_max)
                        
                        if not mask.any():
                            print(f"[background] 警告: {os.path.basename(path)} 在区间 [{B_min}, {B_max}] 没有数据点")
                            error_count += 1
                            continue
                        
                        # 检查数据点数量是否足够
                        n_points = mask.sum()
                        if n_points <= poly_order:
                            print(f"[background] 警告: {os.path.basename(path)} 数据点({n_points})不足以拟合{poly_order}次多项式(需要>{poly_order})")
                            error_count += 1
                            continue
                        
                        # 使用纯处理核心拟合并移除背景
                        background_result = remove_polynomial_background(
                            df[x_col], df[y_col], B_min, B_max, poly_order
                        )
                        values = background_result.values
                        row_positions = list(range(len(df)))
                        print(f"[background] 成功: 去{poly_order}次多项式背景 - {os.path.basename(path)} ({y_col})")
                    changes.append((i, y_col, row_positions, values))
                    success_count += 1
                    
                except ValueError as e:
                    print(f"[background] 错误: {os.path.basename(path)} 输入格式错误 - {e}")
                    error_count += 1
                except Exception as e:
                    print(f"[background] 错误: {os.path.basename(path)} 处理失败 - {e}")
                    error_count += 1
            
            changed = self._commit_column_changes(changes)
            dlg.accept()
            
            # 显示处理结果统计
            result_msg = f"去背景完成({poly_order}次多项式): 成功{success_count}, 跳过{skip_count}"
            if error_count > 0:
                result_msg += f", 失败{error_count}"
            self.statusBar().showMessage(result_msg)
            
            if changed:
                self.replot_all()

        btn_ok.clicked.connect(on_ok)
        btn_cancel.clicked.connect(dlg.reject)
        dlg.exec()

    #重新绘制所有当前曲线（使用当前 combo 中的列
    def replot_all(self, preserve_view=False):
        if not getattr(self, "loaded_files", None):
            self.statusBar().showMessage("尚未加载任何文件", 3000)
            return

        x_col = self.combo_x.currentText()
        y_col = self.combo_y.currentText()
        if not x_col or not y_col:
            self.statusBar().showMessage("请先选择 X/Y 列以重新绘制", 3000)
            return
        
        # 如果需要保留当前视图（缩放/平移），在绘制前保存坐标范围并在绘制后恢复
        cur_xlim = None
        cur_ylim = None
        if preserve_view:
            try:
                cur_xlim = self.ax.get_xlim()
                cur_ylim = self.ax.get_ylim()
            except Exception:
                cur_xlim = None
                cur_ylim = None

        try:
            self._draw_all_files(x_col, y_col)
            # 在成功重绘之后，恢复视图范围（如果之前保存了）
            if preserve_view and (cur_xlim is not None) and (cur_ylim is not None):
                try:
                    self.ax.set_xlim(cur_xlim)
                    self.ax.set_ylim(cur_ylim)
                    # 触发一次绘制以确保视图更新
                    self.canvas.draw()
                except Exception:
                    pass

            self.statusBar().showMessage(f"已重新绘制所有文件(X={x_col}, Y={y_col})", 3000)
        except Exception as e:
            self.statusBar().showMessage(f"绘图时出错：{e}", 5000)
    
    #核心绘图函数：根据当前 loaded_files 绘制曲线并统一样式
    def _draw_all_files(self, x_col, y_col, x_unit_convert=None):
        # 延迟初始化 matplotlib 样式（仅首次绘图时执行）
        _initialize_mpl_style()

        self.ax.clear()
        # 使用空上下文，不禁用 LaTeX，以允许使用上面配置的字体
        with plt.rc_context({}):
            for file_path, df in self.loaded_files:
                if x_col not in df.columns or y_col not in df.columns:
                    continue
                
                # 应用行范围过滤
                filtered_indices = self._get_filtered_rows(df)
                filtered_df = df.loc[filtered_indices]
                
                x_data = pd.to_numeric(filtered_df[x_col], errors='coerce')
                y_data = pd.to_numeric(filtered_df[y_col], errors='coerce')

                # 应用x单位转换
                if x_unit_convert == 'degree_to_radian':
                    x_data = x_data * np.pi / 180.0
                elif x_unit_convert == 'radian_to_degree':
                    x_data = x_data * 180.0 / np.pi

                label_name = os.path.splitext(os.path.basename(file_path))[0]
                # 使用自定义标签，若未设置则使用文件名
                if not hasattr(self, "_legend_labels"):
                    self._legend_labels = {}
                label_display = self._legend_labels.get(label_name, label_name)

                # 主轴绘图
                self.ax.plot(
                    x_data, y_data, marker='o',
                    label=label_display
                )
            
            x_label = f"{x_col}"
            if x_unit_convert == 'degree_to_radian':
                x_label = f"{x_col} (rad.)"
            elif self.x_unit_mode == 'radian':
                x_label = f"{x_col} (rad.)"
            self.ax.set_xlabel(x_label)
            self.ax.set_ylabel(f"{y_col}")
        # 设置刻度参数（只处理主轴）
        self.ax.minorticks_off()
        self.ax.tick_params(axis='both', top=True, bottom=True, left=True, right=True)

        # 将图例放回绘图区内部，使用自动最佳位置
        try:
            if self.legend_config["show"]:
                self.ax.legend(
                    fontsize=self.legend_config["fontsize"],
                    loc=self.legend_config["loc"],
                    frameon=self.legend_config["frameon"],
                    framealpha=self.legend_config["framealpha"]
                )
            else:
                # 移除已有图例
                if self.ax.get_legend() is not None:
                    self.ax.get_legend().remove()
        except Exception:
            self.ax.legend()

        self.figure.subplots_adjust(left=0.15, bottom=0.12, right=0.95, top=0.95)
        self.canvas.draw()

        # 状态栏信息
        x_unicode = self.col_unicode_map.get(x_col, x_col)
        y_unicode = self.col_unicode_map.get(y_col, y_col)
        
        self.statusBar().showMessage(f"绘制完成: {y_unicode} vs {x_unicode}")
    
    def _set_pi_ticks(self):
        """设置x轴刻度为π/2的倍数"""
        try:
            # 获取当前x轴范围
            x_min, x_max = self.ax.get_xlim()
            
            # 计算合适的π倍数刻度
            pi_min = x_min / np.pi
            pi_max = x_max / np.pi
            
            # 生成π/2的倍数刻度（0, π/2, π, 3π/2, 2π等）
            pi_ticks = []
            p = np.ceil(pi_min * 2) / 2  # 从π/2的倍数开始
            while p <= pi_max + 0.01:
                pi_ticks.append(round(p * 2) / 2)  # 精度处理
                p += 0.5
            
            # 移除重复项
            pi_ticks = sorted(set(pi_ticks))
            pi_ticks = [p for p in pi_ticks if pi_min - 0.1 <= p <= pi_max + 0.1]
            
            if not pi_ticks:
                pi_ticks = [pi_min, (pi_min + pi_max) / 2, pi_max]
            
            # 转换为实际坐标
            x_ticks = [p * np.pi for p in pi_ticks]
            
            # 生成刻度标签
            def format_pi_label(p):
                if abs(p) < 1e-10:
                    return '0'
                # 检查是否为π/2的倍数
                for num, denom in [(1, 2), (1, 1), (3, 2), (2, 1), (5, 2), (3, 1)]:
                    if abs(p - num / denom) < 1e-10:
                        if denom == 1:
                            if num == 1:
                                return 'π'
                            else:
                                return f'{num}π'
                        else:
                            if num == 1:
                                return f'π/{denom}'
                            else:
                                return f'{num}π/{denom}'
                # 负数处理
                if p < 0:
                    return '-' + format_pi_label(-p)
                # 默认格式
                return f'{p:.2g}π'
            
            labels = [format_pi_label(p) for p in pi_ticks]
            
            self.ax.set_xticks(x_ticks)
            self.ax.set_xticklabels(labels)
            
        except Exception as e:
            print(f"设置π刻度失败: {e}")

    def _load_placeholder_heart_data(self):
        """加载示例心形数据集并绘制一次（仅启动时使用）。"""
        try:
            t = np.linspace(0, 2 * np.pi, 400)
            x = 16 * np.sin(t) ** 3
            y = 13 * np.cos(t) - 5 * np.cos(2 * t) - 2 * np.cos(3 * t) - np.cos(4 * t)
            df = pd.DataFrame({"x": x, "y": y})
            self.loaded_files = [("heart_placeholder", df)]
            self.history.reset(self.loaded_files)
            self.placeholder_active = True

            # 设置下拉选项为示例列
            try:
                self.combo_x.clear()
                self.combo_y.clear()
                self.combo_x.addItem("x")
                self.combo_y.addItem("y")
                self.combo_x.setCurrentIndex(0)
                self.combo_y.setCurrentIndex(0)
            except Exception:
                pass

            # 绘制示例
            self.replot_all()
            self.statusBar().showMessage("已加载示例心形曲线，打开文件即可替换", 4000)
        except Exception as e:
            self.statusBar().showMessage(f"加载示例数据失败: {e}", 4000)
    
    def _load_placeholder_color_demo(self):
        """加载七条余弦曲线 a*cos(x)，a 从 0.5 到 2.5。"""
        try:
            # x 范围覆盖多周期，方便配色观察
            x = np.linspace(-np.pi, np.pi, 100)
            n = 7
            # a 参数从 0.5 到 2.5
            a_values = np.linspace(0.5, 2.5, n)
            demo_files = []
            for i, a in enumerate(a_values):
                y = a * np.cos(x)
                df = pd.DataFrame({"x": x, "y": y})
                demo_files.append((f"{a:.2f}_cosx", df))

            self.loaded_files = demo_files
            self.history.reset(self.loaded_files)
            self.placeholder_active = True

            # 下拉列设置
            try:
                self.combo_x.clear()
                self.combo_y.clear()
                self.combo_x.addItem("x")
                self.combo_y.addItem("y")
                self.combo_x.setCurrentIndex(0)
                self.combo_y.setCurrentIndex(0)
            except Exception:
                pass

            # 绘制示例
            self.replot_all()
            self.ax.set_xlabel("Angle (rad)")
            self.ax.set_ylabel("Amplitude")
            self.ax.tick_params(axis='both', top=True, right=True)
            self._set_pi_ticks()
            self.statusBar().showMessage("已加载示例曲线（7条 a*cos(x)，a=0.5~2.5），可用于观察配色", 4000)
        except Exception as e:
            self.statusBar().showMessage(f"加载示例数据失败: {e}", 4000)
    def _commit_column_changes(self, changes):
        commands = [
            ColumnPatchCommand.create(self.history, file_position, column, rows, values)
            for file_position, column, rows, values in changes
        ]
        return self.history.execute(CompositeCommand(commands)) if commands else False

    def _commit_row_deletions(self, deletions, preserve_view=False):
        commands = [
            DeleteRowsCommand.create(self.history, file_position, rows)
            for file_position, rows in sorted(deletions.items())
        ]
        changed = self.history.execute(CompositeCommand(commands)) if commands else False
        return changed

    def _commit_file_deletions(self, file_positions):
        return self.history.execute(DeleteFilesCommand.create(self.history, file_positions))

    def _refresh_history_view(self, preserve_view=False):
        self._update_combo_columns()
        if self.loaded_files:
            self.replot_all(preserve_view=preserve_view)
        else:
            self.ax.clear()
            self.canvas.draw()

    def closeEvent(self, event):
        self.interactive_draws.cancel()
        self.tasks.cancel_all()
        if not self.tasks.wait_for_done(3000):
            LOGGER.warning("close_deferred active_tasks=%d", self.tasks.active_count)
            self.statusBar().showMessage("后台任务仍在安全退出，请稍后再关闭")
            event.ignore()
            return
        LOGGER.info("window_closed active_tasks=%d", self.tasks.active_count)
        super().closeEvent(event)

    #撤回上一步操作
    def undo(self):
        try:
            changed = self.history.undo()
        except HistoryError as error:
            self.statusBar().showMessage(f"撤回失败: {error}")
            return
        if not changed:
            self.statusBar().showMessage("没有可撤回的操作")
            return
        self._refresh_history_view()
        self.statusBar().showMessage("已撤回上一步操作")

    def redo(self):
        try:
            changed = self.history.redo()
        except HistoryError as error:
            self.statusBar().showMessage(f"重做失败: {error}")
            return
        if not changed:
            self.statusBar().showMessage("没有可重做的操作")
            return
        self._refresh_history_view()
        self.statusBar().showMessage("已重做上一步操作")

    # 辅助函数：根据当前 loaded_files 更新 combo_x 和 combo_y 的列选项
    def _update_combo_columns(self):
        """根据当前 loaded_files 中的所有文件更新 combo_x 和 combo_y 的列选项"""
        if not self.loaded_files:
            try:
                self.combo_x.clear()
                self.combo_y.clear()
                self.last_x_col = ""
                self.last_y_col = ""
            except Exception:
                pass
            return

        # 收集所有文件中的所有列名（保持顺序）
        all_columns = []
        seen = set()
        for _, df in self.loaded_files:
            for col in df.columns:
                if col not in seen:
                    all_columns.append(col)
                    seen.add(col)

        if not all_columns:
            try:
                self.combo_x.clear()
                self.combo_y.clear()
            except Exception:
                pass
            return

        try:
            # 保存当前选择
            current_x = self.combo_x.currentText()
            current_y = self.combo_y.currentText()

            # 更新下拉菜单
            self.combo_x.clear()
            self.combo_y.clear()
            self.combo_x.addItems(all_columns)
            self.combo_y.addItems(all_columns)

            # 尝试恢复之前的选择（如果列仍然存在）
            if current_x in all_columns:
                self.combo_x.setCurrentText(current_x)
            else:
                # 如果之前的选择不存在，使用第一列
                self.combo_x.setCurrentText(all_columns[0])

            if current_y in all_columns:
                self.combo_y.setCurrentText(current_y)
            else:
                # 如果之前的选择不存在，使用第二列（或第一列如果只有一列）
                self.combo_y.setCurrentText(all_columns[1] if len(all_columns) > 1 else all_columns[0])

        except Exception as e:
            print(f"更新 combo 列表时出错: {e}")

    #导出当前加载的数据到文件（Excel/CSV/TXT）
    
    def _show_column_selection_dialog(self):
        """显示列选择对话框，返回用户选中的列列表，或 None 如果取消。"""
        return choose_export_columns(self, self.loaded_files)


    def export_data(self):
        """Collect GUI choices and delegate export data and file rules to the core."""
        if not self.loaded_files:
            self.statusBar().showMessage("没有数据可以导出")
            return

        selected_columns = self._show_column_selection_dialog()
        if selected_columns is None:
            return

        options = QFileDialog.Options() | QFileDialog.DontConfirmOverwrite
        file_path, _ = QFileDialog.getSaveFileName(
            self,
            "导出数据",
            "",
            "CSV Files (*.csv);;Excel Files (*.xlsx);;Text Files (*.txt)",
            options=options,
        )
        if not file_path:
            return

        mode = "overwrite"
        if os.path.exists(file_path):
            mode = choose_existing_file_mode(self, file_path)
            if mode is None:
                return

        try:
            fitted_curves = [
                FittedCurve(
                    name=f"fitted_curve_{index + 1}",
                    x=line.get_xdata(),
                    y=line.get_ydata(),
                )
                for index, line in enumerate(self.fitted_lines)
            ]
        except Exception as error:
            export_error = DataIOError(
                path=file_path,
                operation="export",
                code="fitted_curve_read_failed",
                reason=str(error),
            )
            self.statusBar().showMessage(f"导出数据失败: {export_error}")
            LOGGER.error("export_failed error=%s", export_error)
            return

        try:
            bundle = prepare_export(
                [ExportSource(path, frame) for path, frame in self.loaded_files],
                fitted_curves,
                selected_columns,
                self.combo_x.currentText(),
                self.x_unit_mode,
                os.path.splitext(file_path)[1],
            )
            write_export(file_path, bundle, mode)
        except DataIOError as error:
            self.statusBar().showMessage(f"导出数据失败: {error}")
            LOGGER.error("export_failed error=%s", error)
            return

        self.statusBar().showMessage(f"数据导出成功: {file_path}")

    
    def apply_light_theme(self):
        app = QApplication.instance()
        if app is None:
            return
        # 恢复轻主题（清空样式表或重置为默认）
        try:
            # 尝试从文件加载浅色样式表
            qss_path = os.path.join(os.path.dirname(__file__), 'style_light.qss')
            if os.path.exists(qss_path):
                with open(qss_path, 'r', encoding='utf-8') as f:
                    app.setStyleSheet(f.read())
            else:
                app.setStyleSheet('')
        except Exception:
            pass
        try:
            plt.style.use('seaborn-v0_8-paper')
        except Exception:
            pass
        # 绘图区设为白色
        self.figure.patch.set_facecolor('white')
        self.ax.set_facecolor('white')
        # 提高浅色主题下网格和坐标轴的对比度，避免网格太淡
        try:
            # 设置轴线颜色（深灰），刻度颜色，以及网格颜色
            for spine in self.ax.spines.values():
                spine.set_color('#000000')
                spine.set_linewidth(1.6)
            self.ax.xaxis.label.set_color('#222222')
            self.ax.yaxis.label.set_color('#222222')
            # 在四周显示刻度，刻度向内，颜色为深色（这些参数会在绘图时统一设置，这里不重复设置）
            # self.ax.tick_params(axis='both', which='major', colors='#000000', direction='in', top=True, right=True, width=1.6)
            # self.ax.tick_params(axis='both', which='minor', colors='#000000', direction='in', top=True, right=True, width=1.2)
            self.ax.grid(True, color='#cccccc', linestyle='--', alpha=0.9)
            # 更新已存在的图例边框颜色（如果存在）
            legend = self.ax.get_legend()
            if legend is not None:
                legend.get_frame().set_edgecolor('#000000')
                legend.get_frame().set_linewidth(0.8)
        except Exception:
            pass
        # 恢复顶部按钮浅色样式
        try:
            for btn in [self.btn_denoise, self.btn_local_detrend, self.btn_row_filter, self.btn_center, self.btn_normalize, self.btn_remove_bg, self.btn_clear, self.btn_save, self.btn_plot]:
                btn.setStyleSheet(self.top_button_style)
        except Exception:
            pass
        # 恢复下拉为浅色
        try:
            self.combo_x.setStyleSheet(self.combo_style_light)
            self.combo_y.setStyleSheet(self.combo_style_light)
        except Exception:
            pass
    # 深色主题支持已移除
        # 强制重绘画布以立即反映浅色背景
        try:
            self.canvas.draw()
        except Exception:
            pass
        self.replot_all()

#纵坐标对称以及归一化数据
def center_data(Y):
    return center_values(Y).values

def normalize_data(Y, top_n=20):
    try:
        result = normalize_values(Y, top_n=top_n)
    except ProcessingError as error:
        if error.code in {"empty_values", "no_finite_values"}:
            values = np.asarray(pd.to_numeric(Y, errors="coerce"), dtype=float)
            return values, np.nan
        raise
    return result.values, result.metadata["scale"]

def local_flatten_keep_anchor(x, y, x1, x2, transition=0, anchor='left', strength=1.0):
    """
    在 [x1, x2] 内做局部去斜率，使该段更平，
    同时保持 anchor 端点不动，并可选平滑过渡。

    参数：
    - x, y: array-like，原始数据
    - x1, x2: float，处理区间
    - transition: float，两侧过渡宽度，0 表示无过渡
    - anchor: {'left', 'right', 'center'}，固定哪一端
    - strength: float，处理强度，0~1。1 表示完全去掉拟合斜率

    返回：
    - y_new: 处理后的数据
    """
    return local_flatten_values(
        x, y, x1, x2, transition=transition, anchor=anchor, strength=strength
    ).values

def denoise_data(df, y_col='rem', window_length=11, polyorder=3, x_col=None, x1=None, x2=None):
    """
    使用 Savitzky-Golay 滤波对数据进行去噪处理。
    
    参数：
    - df: pd.DataFrame，包含要去噪的列
    - y_col: str，要去噪的列名
    - window_length: int，Savitzky-Golay 滤波窗口大小（必须是奇数）
    - polyorder: int，Savitzky-Golay 拟合多项式阶数
    - x_col: str，横坐标列名（可选，用于区间处理）
    - x1, x2: float，处理区间（如果指定，只处理该区间内的数据）
    
    返回：
    - 去噪后的数据数组
    """
    # 兼容旧入口：历史行为会把偶数窗口向上调整为奇数；纯核心则严格拒绝偶数窗口。
    if (
        isinstance(window_length, (int, np.integer))
        and not isinstance(window_length, (bool, np.bool_))
        and window_length % 2 == 0
    ):
        window_length += 1
    try:
        result = denoise_values(
            df[y_col].to_numpy(),
            window_length,
            polyorder,
            x=None if x_col is None else df[x_col].to_numpy(),
            x1=x1,
            x2=x2,
        )
    except ProcessingError as error:
        if error.code == "invalid_window":
            return df[y_col].copy()
        raise
    if x_col is not None:
        return pd.Series(result.values, index=df.index, name=y_col)
    return result.values


if __name__ == "__main__":
    log_path = configure_logging()
    # 兼容在嵌入/交互环境中已存在 QApplication 的情况
    app = QApplication.instance() or QApplication(sys.argv)
    sys.excepthook = make_exception_hook(
        lambda title, message, details: show_error_details(
            QApplication.activeWindow(), title, message, details
        )
    )
    LOGGER.info("application_start log_path=%s", log_path)
    
    # 创建主窗口（窗口尺寸和位置已在 __init__ 中自动设置）
    window = PlotApp()
    
    window.show()
    sys.exit(app.exec())
