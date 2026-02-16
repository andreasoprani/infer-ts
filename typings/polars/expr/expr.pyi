from infer_ts.namespace import ExprInferTsNamespace

class Expr:
    @property
    def infer_ts(self) -> ExprInferTsNamespace: ...
