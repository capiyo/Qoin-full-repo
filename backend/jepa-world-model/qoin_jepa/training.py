"""Leakage-agnostic training primitive for the JEPA research model.

The caller is responsible for constructing chronological context/future examples
without future leakage and fitting all scaling on past/training data only.
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
    *,
    variance_weight: float = 0.05,
    covariance_weight: float = 0.01,
    max_grad_norm: float = 1.0,
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
    loss = latent_prediction_loss(
        predicted, target,
        variance_weight=variance_weight,
        covariance_weight=covariance_weight,
    )
    if not torch.isfinite(loss):
        optimizer.zero_grad(set_to_none=True)
        raise ValueError("latent prediction loss is non-finite")
    loss.backward()
    torch.nn.utils.clip_grad_norm_(
        [parameter for parameter in model.parameters() if parameter.requires_grad],
        max_norm=max_grad_norm,
        error_if_nonfinite=True,
    )
    optimizer.step()
    model.update_target_encoder()
    return loss.detach()
