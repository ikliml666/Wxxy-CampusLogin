import { Component, type ReactNode } from 'react'
import { Button } from '@/components/ui/button'
import { AlertTriangle } from 'lucide-react'

interface Props {
  children: ReactNode
}

interface State {
  hasError: boolean
  error: Error | null
}

export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props)
    this.state = { hasError: false, error: null }
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error }
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    console.error('[ErrorBoundary] 渲染错误:', error, errorInfo)
  }

  render() {
    if (this.state.hasError) {
      return (
        <div className="flex flex-col items-center justify-center h-screen p-8 font-sans text-muted-foreground text-center bg-background">
          <AlertTriangle className="h-12 w-12 mb-4 text-amber-500" />
          <h2 className="text-xl font-semibold text-foreground mb-2">页面渲染出错</h2>
          <p className="text-sm max-w-md leading-relaxed mb-6">
            {this.state.error?.message || '未知错误'}
          </p>
          <Button
            variant="outline"
            onClick={() => {
              this.setState({ hasError: false, error: null })
              window.location.reload()
            }}
          >
            重新加载
          </Button>
        </div>
      )
    }

    return this.props.children
  }
}
