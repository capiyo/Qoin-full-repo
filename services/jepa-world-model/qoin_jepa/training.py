"""Leakage-agnostic training primitive for the JEPA research model.

The caller is responsible for constructing chronological, non-overlapping
context/future examples and fitting all scaling on past/training data only.
"""

from __future__ import annotations

import torch
from torch import Tensor

from .model import MarketJEPA, latent_prediction_loss


def train_batch(
    model: MarketJEPA,
    optimizer: torch.optim.Optimizer,
    context: Tensor,
    future: Tensor,
    horizon: int,
) -> Tensor:
    """Perform one optimizer step and update the EMA target encoder.

    Returns a detached scalar loss. This function does not create samples,
    normalize data, choose splits, or imply that the model has trading value.
    """
    if not torch.isfinite(context).all() or not torch.isfinite(future).all():
        raise ValueError("training batch contains non-finite feature values")
    model.train()
    optimizer.zero_grad(set_to_none=True)
    predicted, target = model(context, future, horizon)
    if target is None:
        raise RuntimeError("future target embeddings were not produced")
    loss = latent_prediction_loss(predicted, target)
    if not torch.isfinite(loss):
        optimizer.zero_grad(set_to_none=True)
        raise ValueError("latent prediction loss is non-finite")
    loss.backward()
    optimizer.step()
    model.update_target_encoder()
    return loss.detach()
