"""A compact JEPA-inspired latent predictor for market feature sequences.

This is a research scaffold, not Meta's JEPA/V-JEPA implementation. Inputs must
be causal, normalized with past-only statistics, and timestamp-aligned.
"""

from __future__ import annotations

import copy

import torch
from torch import Tensor, nn
import torch.nn.functional as F


class FeatureEncoder(nn.Module):
    """Encode each observed market feature vector into a latent representation."""

    def __init__(self, feature_dim: int, latent_dim: int, hidden_dim: int) -> None:
        super().__init__()
        if feature_dim <= 0 or latent_dim <= 0 or hidden_dim <= 0:
            raise ValueError("feature_dim, latent_dim, and hidden_dim must be positive")
        # Inputs are normalized causally by the dataset scaler. Avoid per-row
        # feature normalization here because it would erase cross-feature magnitude.
        self.network = nn.Sequential(
            nn.Linear(feature_dim, hidden_dim),
            nn.GELU(),
            nn.Linear(hidden_dim, latent_dim),
            nn.LayerNorm(latent_dim),
        )

    def forward(self, features: Tensor) -> Tensor:
        return self.network(features)


class LatentPredictor(nn.Module):
    """Predict future latent states conditioned on encoded context and horizon."""

    def __init__(self, latent_dim: int, hidden_dim: int, max_horizon: int) -> None:
        super().__init__()
        if max_horizon <= 0:
            raise ValueError("max_horizon must be positive")
        self.horizon_embedding = nn.Embedding(max_horizon, latent_dim)
        self.network = nn.Sequential(
            nn.LayerNorm(latent_dim * 2),
            nn.Linear(latent_dim * 2, hidden_dim),
            nn.GELU(),
            nn.Linear(hidden_dim, latent_dim),
        )
        self.max_horizon = max_horizon

    def forward(self, context_latent: Tensor, horizon: int) -> Tensor:
        if not 1 <= horizon <= self.max_horizon:
            raise ValueError(f"horizon must be in [1, {self.max_horizon}]")
        horizon_ids = torch.full(
            (context_latent.shape[0],),
            horizon - 1,
            dtype=torch.long,
            device=context_latent.device,
        )
        horizon_latent = self.horizon_embedding(horizon_ids)
        return self.network(torch.cat((context_latent, horizon_latent), dim=-1))


class MarketJEPA(nn.Module):
    """Context encoder + latent predictor + EMA target encoder.

    Training call:
        predicted, target = model(context, future, horizon)
        loss = latent_prediction_loss(predicted, target)

    Shapes:
        context: [batch, context_steps, feature_dim]
        future:  [batch, feature_dim] (the target observation at the chosen horizon)
    """

    def __init__(
        self,
        feature_dim: int,
        latent_dim: int = 64,
        hidden_dim: int = 128,
        max_horizon: int = 8,
        ema_decay: float = 0.996,
    ) -> None:
        super().__init__()
        if not 0.0 < ema_decay < 1.0:
            raise ValueError("ema_decay must be strictly between 0 and 1")
        self.encoder = FeatureEncoder(feature_dim, latent_dim, hidden_dim)
        # Preserve temporal order and learn how recent latent states evolve.
        self.temporal_encoder = nn.GRU(latent_dim, latent_dim, batch_first=True)
        self.target_encoder = copy.deepcopy(self.encoder)
        for parameter in self.target_encoder.parameters():
            parameter.requires_grad_(False)
        self.predictor = LatentPredictor(latent_dim, hidden_dim, max_horizon)
        self.ema_decay = ema_decay

    def forward(
        self, context: Tensor, future: Tensor | None = None, horizon: int = 1
    ) -> tuple[Tensor, Tensor | None]:
        if context.ndim != 3:
            raise ValueError("context must have shape [batch, time, feature_dim]")
        if context.shape[1] == 0:
            raise ValueError("context must contain at least one timestep")
        context_latents = self.encoder(context)
        temporal_latents, _ = self.temporal_encoder(context_latents)
        predicted = self.predictor(temporal_latents[:, -1, :], horizon)
        if future is None:
            return predicted, None
        if future.ndim != 2 or future.shape[0] != context.shape[0]:
            raise ValueError("future must have shape [batch, feature_dim]")
        with torch.no_grad():
            target = self.target_encoder(future).detach()
        return predicted, target

    @torch.no_grad()
    def update_target_encoder(self) -> None:
        """EMA-update target parameters after an optimizer step."""
        for target_parameter, online_parameter in zip(
            self.target_encoder.parameters(), self.encoder.parameters(), strict=True
        ):
            target_parameter.mul_(self.ema_decay).add_(
                online_parameter, alpha=1.0 - self.ema_decay
            )
        for target_buffer, online_buffer in zip(
            self.target_encoder.buffers(), self.encoder.buffers(), strict=True
        ):
            target_buffer.copy_(online_buffer)


def latent_prediction_loss(
    predicted: Tensor,
    target: Tensor,
    *,
    variance_weight: float = 0.05,
    covariance_weight: float = 0.01,
    variance_floor: float = 1.0,
) -> Tensor:
    """Robust JEPA loss with anti-collapse regularization.

    Huber prediction loss is the main objective. Batch-level variance and
    off-diagonal covariance penalties discourage collapsed/redundant latents.
    Regularizers are disabled automatically for batch size < 2 because batch
    statistics are not meaningful there. Targets are always detached.
    """
    if predicted.shape != target.shape:
        raise ValueError("predicted and target latent shapes must match")
    if predicted.ndim != 2 or predicted.shape[0] == 0 or predicted.shape[1] == 0:
        raise ValueError("latent tensors must have shape [batch, latent_dim]")
    if variance_weight < 0 or covariance_weight < 0 or variance_floor <= 0:
        raise ValueError("regularization weights must be non-negative and variance_floor positive")
    target = target.detach()
    prediction = F.smooth_l1_loss(predicted, target)
    if predicted.shape[0] < 2 or (variance_weight == 0 and covariance_weight == 0):
        return prediction

    centered = predicted - predicted.mean(dim=0, keepdim=True)
    std = torch.sqrt(centered.var(dim=0, unbiased=False) + 1e-4)
    variance_penalty = F.relu(variance_floor - std).pow(2).mean()

    covariance = centered.T @ centered / max(predicted.shape[0] - 1, 1)
    off_diagonal = covariance - torch.diag(torch.diagonal(covariance))
    covariance_penalty = off_diagonal.pow(2).sum() / predicted.shape[1]

    return prediction + variance_weight * variance_penalty + covariance_weight * covariance_penalty
