"""Add posts table

Revision ID: 002
"""
from alembic import op
import sqlalchemy as sa

def upgrade():
    op.create_table('posts',
        sa.Column('id', sa.Integer(), nullable=False),
        sa.Column('title', sa.String(), nullable=False),
    )

def downgrade():
    op.drop_table('posts')
